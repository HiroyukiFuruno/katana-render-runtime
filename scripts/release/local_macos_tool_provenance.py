"""Authenticate optional host tools from official archives, without executing them."""

from __future__ import annotations

import hashlib
import gzip
import io
import json
import os
import re
import stat
import struct
import tarfile
from pathlib import Path, PurePosixPath
from typing import Any, Callable, NamedTuple
from urllib import error, parse, request


TOOL_PROVENANCE_POLICY = "official-native-tool-closure-v1"
HOST_TCB_POLICY = "owner-trusted-collector-python-stdlib-and-fixed-apple-os-development-tools-v1"
FORMULA_API = "https://formulae.brew.sh/api/formula/"
JAVA_RELEASE_API = "https://api.github.com/repos/adoptium/temurin21-binaries/releases/latest"
CELLAR = "/opt/homebrew/Cellar"
JAVA_ANCHOR = "/__official_temurin_home__"
MAX_METADATA_BYTES = 8 * 1024 * 1024
MAX_ARCHIVE_BYTES = 256 * 1024 * 1024
MAX_TOTAL_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024
MAX_FILE_BYTES = 512 * 1024 * 1024
MAX_CONTENT_BYTES = 4 * 1024 * 1024 * 1024
MAX_ENTRIES = 200_000
MAX_FORMULAS = 96
MAX_LOAD_COMMANDS = 8192
MAX_UNCOMPRESSED_ARCHIVE_BYTES = 1024 * 1024 * 1024
NAME_RE = re.compile(r"[a-z0-9][a-z0-9+@._-]{0,79}\Z")
VERSION_RE = re.compile(r"[0-9][A-Za-z0-9.+_-]{0,79}\Z")
SHA_RE = re.compile(r"[0-9a-f]{64}\Z")


class ProvenanceError(Exception):
    """An unsupported tool installation must use hosted macOS CI instead."""


class InventoryEntry(NamedTuple):
    kind: str
    mode: int
    value: str


class HostToolProof(NamedTuple):
    graphviz_files: dict[str, InventoryEntry]
    java_files: dict[str, InventoryEntry]
    identities: dict[str, Any]
    dot_path: str


class _Budget:
    def __init__(self) -> None:
        self.archives = 0
        self.content = 0
        self.entries = 0


def _canonical(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def _sha(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ProvenanceError("official metadata contains duplicate keys")
        result[key] = value
    return result


def _read_url(url: str, limit: int, headers: dict[str, str] | None = None) -> bytes:
    allowed = {"formulae.brew.sh", "api.github.com", "github.com", "release-assets.githubusercontent.com", "ghcr.io", "pkg-containers.githubusercontent.com"}
    initial = parse.urlparse(url)
    if initial.scheme != "https" or initial.hostname not in allowed or initial.username or initial.password:
        raise ProvenanceError("official artifact URL is unsupported")
    try:
        req = request.Request(url, headers={"User-Agent": "krr-local-tool-provenance", **(headers or {})})
        with request.urlopen(req, timeout=30) as response:
            final = parse.urlparse(response.geturl())
            if final.scheme != "https" or final.hostname not in allowed or final.username or final.password:
                raise ProvenanceError("official artifact redirected to an unsupported host")
            value = response.read(limit + 1)
    except (OSError, error.URLError) as exc:
        raise ProvenanceError("official artifact could not be retrieved") from exc
    if not value or len(value) > limit:
        raise ProvenanceError("official artifact exceeds its bounded size")
    return value


def fetch_official_json(url: str) -> Any:
    try:
        return json.loads(_read_url(url, MAX_METADATA_BYTES), object_pairs_hook=_unique_object)
    except (ValueError, UnicodeError) as exc:
        raise ProvenanceError("official metadata is invalid JSON") from exc


def fetch_official_archive(url: str) -> bytes:
    headers: dict[str, str] = {}
    if url.startswith("https://ghcr.io/v2/homebrew/core/"):
        match = re.fullmatch(r"https://ghcr\.io/v2/homebrew/core/([a-z0-9+._-]+(?:/[0-9]+)?)/blobs/sha256:([0-9a-f]{64})", url)
        if match is None:
            raise ProvenanceError("Homebrew bottle URL is invalid")
        # WHY: 公開bottleもGHCRの匿名pull tokenが必要で、ローカルBrew認証には依存しない。
        token_url = "https://ghcr.io/token?" + parse.urlencode({
            "service": "ghcr.io", "scope": f"repository:homebrew/core/{match.group(1)}:pull",
        })
        token = fetch_official_json(token_url)
        if not isinstance(token, dict) or not isinstance(token.get("token"), str) or not token["token"]:
            raise ProvenanceError("public Homebrew bottle token is unavailable")
        headers["Authorization"] = "Bearer " + token["token"]
    return _read_url(url, MAX_ARCHIVE_BYTES, headers)


def _name(value: Any) -> str:
    if not isinstance(value, str) or NAME_RE.fullmatch(value) is None:
        raise ProvenanceError("Homebrew formula name is unsupported")
    return value


def _names(value: Any) -> list[str]:
    if not isinstance(value, list):
        raise ProvenanceError("Homebrew runtime dependency metadata is unsupported")
    names = [_name(item) for item in value]
    if len(names) != len(set(names)):
        raise ProvenanceError("Homebrew runtime dependency metadata is ambiguous")
    return names


def _formula(value: Any, name: str, tag: str) -> tuple[dict[str, Any], list[str]]:
    if not isinstance(value, dict) or value.get("name") != name or value.get("full_name") != name:
        raise ProvenanceError("Homebrew formula identity does not match the official request")
    versions = value.get("versions")
    revision = value.get("revision")
    if (not isinstance(versions, dict) or not isinstance(versions.get("stable"), str)
            or VERSION_RE.fullmatch(versions["stable"]) is None
            or type(revision) is not int or revision < 0 or revision > 100_000):
        raise ProvenanceError("Homebrew stable version metadata is unsupported")
    version = versions["stable"] + (f"_{revision}" if revision else "")
    deps = _names(value.get("dependencies")) + _names(value.get("recommended_dependencies", []))
    if len(deps) != len(set(deps)):
        raise ProvenanceError("Homebrew dependency classes overlap")
    runtime = value.get("runtime_dependencies")
    if runtime is not None:
        if not isinstance(runtime, list):
            raise ProvenanceError("Homebrew runtime dependency metadata is unsupported")
        direct: list[str] = []
        for item in runtime:
            if (not isinstance(item, dict) or type(item.get("declared_directly")) is not bool
                    or not isinstance(item.get("version"), str)
                    or VERSION_RE.fullmatch(item["version"]) is None):
                raise ProvenanceError("Homebrew runtime dependency entry is unsupported")
            dep = _name(item.get("full_name"))
            if item["declared_directly"]:
                direct.append(dep)
        if set(direct) != set(deps) or len(direct) != len(set(direct)):
            raise ProvenanceError("Homebrew runtime dependency metadata is inconsistent")
    try:
        stable = value["bottle"]["stable"]
        bottle_files = stable["files"]
        selected_tag = tag if tag in bottle_files else "all"
        selected = bottle_files[selected_tag]
    except (KeyError, TypeError) as exc:
        raise ProvenanceError("native macOS bottle is unavailable") from exc
    if (not isinstance(stable, dict) or stable.get("root_url") != "https://ghcr.io/v2/homebrew/core"
            or not isinstance(selected, dict) or not isinstance(selected.get("sha256"), str)
            or SHA_RE.fullmatch(selected["sha256"]) is None
            or selected.get("cellar") not in {CELLAR, ":any", ":any_skip_relocation"}):
        raise ProvenanceError("Homebrew bottle metadata is unsupported")
    expected_url = f"https://ghcr.io/v2/homebrew/core/{name.replace('@', '/')}/blobs/sha256:{selected['sha256']}"
    if selected.get("url") != expected_url:
        raise ProvenanceError("Homebrew bottle URL does not match its official digest")
    return {"name": name, "version": version, "bottle_tag": selected_tag, "url": expected_url,
            "sha256": selected["sha256"]}, deps


def _java_release(value: Any) -> dict[str, str]:
    if (not isinstance(value, dict) or value.get("draft") is not False or value.get("prerelease") is not False
            or not isinstance(value.get("tag_name"), str)
            or re.fullmatch(r"jdk-21\.\d+\.\d+(?:\.\d+)?\+\d+", value["tag_name"]) is None
            or not isinstance(value.get("assets"), list)):
        raise ProvenanceError("official Temurin 21 release metadata is unsupported")
    matches = [item for item in value["assets"] if isinstance(item, dict)
               and isinstance(item.get("name"), str)
               and re.fullmatch(r"OpenJDK21U-jdk_aarch64_mac_hotspot_[0-9]+(?:\.[0-9]+){2,3}_[0-9]+\.tar\.gz", item["name"])]
    if len(matches) != 1:
        raise ProvenanceError("official Temurin native JDK asset is ambiguous")
    asset = matches[0]
    digest = asset.get("digest")
    release_version = value["tag_name"].removeprefix("jdk-").replace("+", "_")
    expected_name = f"OpenJDK21U-jdk_aarch64_mac_hotspot_{release_version}.tar.gz"
    expected_url = "https://github.com/adoptium/temurin21-binaries/releases/download/" + parse.quote(value["tag_name"], safe="") + "/" + expected_name
    if (asset["name"] != expected_name or asset.get("browser_download_url") != expected_url
            or not isinstance(digest, str) or re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is None
            or type(asset.get("size")) is not int or not 0 < asset["size"] <= MAX_ARCHIVE_BYTES):
        raise ProvenanceError("official Temurin native JDK digest or URL is invalid")
    return {"release": value["tag_name"], "asset": expected_name, "url": expected_url,
            "sha256": digest.removeprefix("sha256:")}


def _safe_path(value: str) -> PurePosixPath:
    path = PurePosixPath(value)
    if (not value or "\x00" in value or "\\" in value or path.is_absolute()
            or any(part in {".", ".."} for part in value.split("/"))
            or any(not part for part in value.rstrip("/").split("/"))):
        raise ProvenanceError("official archive contains an unsafe path")
    return path


def _link_target(source: str, target: str, boundary: str) -> str:
    if not target or "\x00" in target or "\\" in target or target.startswith("/"):
        raise ProvenanceError("official archive contains an unsafe link")
    parts = list(PurePosixPath(source).parent.parts)
    for part in target.split("/"):
        if part in {"", "."}:
            continue
        if part == "..":
            if not parts:
                raise ProvenanceError("official archive link escapes its installation")
            parts.pop()
        else:
            parts.append(part)
    normalized = PurePosixPath(*parts).as_posix()
    if normalized != boundary and not normalized.startswith(boundary + "/"):
        raise ProvenanceError("official archive link escapes its installation")
    return normalized


def _archive_inventory(
    archive: bytes, digest: str, prefix: str, budget: _Budget,
) -> tuple[dict[str, InventoryEntry], dict[str, tuple[list[str], list[str]]]]:
    if not isinstance(archive, bytes) or not archive or len(archive) > MAX_ARCHIVE_BYTES or _sha(archive) != digest:
        raise ProvenanceError("official archive does not match its independent digest")
    budget.archives += len(archive)
    if budget.archives > MAX_TOTAL_ARCHIVE_BYTES:
        raise ProvenanceError("official tool closure exceeds its archive budget")
    entries: dict[str, InventoryEntry] = {}
    natives: dict[str, tuple[list[str], list[str]]] = {}
    seen: set[str] = set()
    class BoundedGzip:
        def __init__(self) -> None:
            self.source = gzip.GzipFile(fileobj=io.BytesIO(archive))
            self.consumed = 0

        def read(self, size: int) -> bytes:
            if not 0 <= size <= MAX_FILE_BYTES:
                raise ProvenanceError("official archive requests an unsupported read size")
            content = self.source.read(min(size, MAX_UNCOMPRESSED_ARCHIVE_BYTES - self.consumed + 1))
            self.consumed += len(content)
            if self.consumed > MAX_UNCOMPRESSED_ARCHIVE_BYTES:
                raise ProvenanceError("official archive exceeds its decompressed byte bound")
            return content

    reader = BoundedGzip()
    try:
        with tarfile.open(fileobj=reader, mode="r|") as source:
            for member in source:
                budget.entries += 1
                if budget.entries > MAX_ENTRIES:
                    raise ProvenanceError("official tool closure exceeds its entry budget")
                path = _safe_path(member.name).as_posix()
                if path in seen:
                    raise ProvenanceError("official archive contains duplicate paths")
                seen.add(path)
                if member.size < 0 or member.size > MAX_FILE_BYTES:
                    raise ProvenanceError("official archive file has unsupported mode or size")
                if member.isdir():
                    # WHY: 公式JDKのsetgid directoryはgroup継承属性であり、ファイルの特殊実行権限ではない。
                    if member.mode & 0o5000:
                        raise ProvenanceError("official archive directory has privileged mode")
                    continue
                if member.mode & 0o7000:
                    raise ProvenanceError("official archive file has privileged mode")
                if not path.startswith(prefix + "/"):
                    raise ProvenanceError("official archive file is outside its expected installation")
                relative = path.removeprefix(prefix + "/")
                if relative == "INSTALL_RECEIPT.json" and not member.isfile():
                    raise ProvenanceError("installer metadata is not a regular nonexecuting file")
                if member.issym():
                    target = _link_target(path, member.linkname, prefix)
                    entries[relative] = InventoryEntry("symlink", 0, target.removeprefix(prefix + "/") if target != prefix else ".")
                elif member.isfile():
                    budget.content += member.size
                    if budget.content > MAX_CONTENT_BYTES:
                        raise ProvenanceError("official tool closure exceeds its content budget")
                    stream = source.extractfile(member)
                    if stream is None:
                        raise ProvenanceError("official archive file cannot be read")
                    content = stream.read(member.size + 1)
                    if len(content) != member.size:
                        raise ProvenanceError("official archive file length is inconsistent")
                    if relative == "INSTALL_RECEIPT.json":
                        # WHY: pour後に変わるreceiptは実行されず、真正性の根拠にも使用しない。
                        if member.mode & 0o111 or member.size > MAX_METADATA_BYTES:
                            raise ProvenanceError("installer metadata has an executable mode or excessive size")
                        entries[relative] = InventoryEntry("installer-metadata", 0, "")
                    else:
                        entries[relative] = InventoryEntry("file", member.mode & 0o111, _sha(content))
                    native = _native_dependencies(content)
                    if native is not None:
                        natives[relative] = native
                else:
                    raise ProvenanceError("official archive contains unsupported special or hard-linked entries")
    except (tarfile.TarError, OSError, EOFError, ValueError, struct.error) as exc:
        raise ProvenanceError("official archive could not be safely parsed") from exc
    finally:
        reader.source.close()
    if not entries:
        raise ProvenanceError("official archive inventory is empty")
    return entries, natives


def _native_dependencies(content: bytes) -> tuple[list[str], list[str]] | None:
    magics = {b"\xcf\xfa\xed\xfe": ("<", 32), b"\xfe\xed\xfa\xcf": (">", 32),
              b"\xce\xfa\xed\xfe": ("<", 28), b"\xfe\xed\xfa\xce": (">", 28)}
    if content[:4] == b"\xca\xfe\xba\xbe" and len(content) >= 10:
        # WHY: Java classとfat Mach-Oはmagicが同じだが、classのmajor versionはfat arch数ではない。
        major = struct.unpack_from(">H", content, 6)[0]
        if 45 <= major <= 100:
            return None
    if content[:4] in {b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca", b"\xca\xfe\xba\xbf", b"\xbf\xba\xfe\xca"}:
        raise ProvenanceError("universal native tool archives require an unsupported architecture parser")
    if content[:4] not in magics:
        return None
    endian, header_size = magics[content[:4]]
    if len(content) < header_size:
        raise ProvenanceError("native tool Mach-O header is truncated")
    cpu, _subtype, _kind, count, command_bytes = struct.unpack_from(endian + "IIIII", content, 4)
    if cpu != 0x0100000C or count > MAX_LOAD_COMMANDS or header_size + command_bytes > len(content):
        raise ProvenanceError("native tool Mach-O architecture or load commands are unsupported")
    dependencies: list[str] = []
    rpaths: list[str] = []
    position = header_size
    dylib_commands = {0xC, 0x18 | 0x80000000, 0x1F | 0x80000000, 0x20, 0x23 | 0x80000000}
    for _ in range(count):
        if position + 8 > header_size + command_bytes:
            raise ProvenanceError("native tool load command is truncated")
        command, size = struct.unpack_from(endian + "II", content, position)
        if size < 8 or size % 4 or position + size > header_size + command_bytes:
            raise ProvenanceError("native tool load command size is invalid")
        if command in {0x6, 0x10, 0x27}:
            raise ProvenanceError("native tool has an unsupported legacy binding or loader environment")
        if command in dylib_commands or command in {0x8000001C, 0xE}:
            minimum = 24 if command in dylib_commands else 12
            if size < minimum:
                raise ProvenanceError("native tool load path command is truncated")
            offset = struct.unpack_from(endian + "I", content, position + 8)[0]
            if not minimum <= offset < size:
                raise ProvenanceError("native tool load path offset is invalid")
            raw = content[position + offset:position + size]
            end = raw.find(b"\0")
            if end < 0:
                raise ProvenanceError("native tool load path is unterminated")
            try:
                path = raw[:end].decode("utf-8")
            except UnicodeError as exc:
                raise ProvenanceError("native tool load path is invalid") from exc
            if not path or "\\" in path:
                raise ProvenanceError("native tool load path is invalid")
            (rpaths if command == 0x8000001C else dependencies).append(path)
        position += size
    if position != header_size + command_bytes:
        raise ProvenanceError("native tool load command inventory is inconsistent")
    return dependencies, rpaths


def resolve_inventory_path(files: dict[str, InventoryEntry], path: str) -> str:
    current = path
    seen: set[str] = set()
    for _ in range(64):
        if current in seen:
            raise ProvenanceError("official inventory contains a link cycle")
        seen.add(current)
        entry = files.get(current)
        if entry is not None and entry.kind == "file":
            return current
        if entry is not None and entry.kind == "symlink":
            current = entry.value
            continue
        aliases = [key for key, value in files.items() if value.kind == "symlink" and current.startswith(key + "/")]
        if not aliases:
            raise ProvenanceError("native dependency is outside the authenticated inventory")
        alias = max(aliases, key=len)
        current = files[alias].value + current[len(alias):]
    raise ProvenanceError("official inventory link chain exceeds its bound")


def _normal_path(path: str) -> str:
    if not path.startswith("/"):
        raise ProvenanceError("native dependency path is not absolute")
    normalized = os.path.normpath(path)
    if "\x00" in normalized or "\\" in normalized:
        raise ProvenanceError("native dependency path is invalid")
    return normalized


def _expand_loader(path: str, owner: str, executable: str) -> str:
    for token, base in (("@loader_path", str(PurePosixPath(owner).parent)),
                        ("@executable_path", str(PurePosixPath(executable).parent))):
        if path == token or path.startswith(token + "/"):
            return _normal_path(base + path[len(token):])
    return _normal_path(path)


def _verify_native_closure(
    files: dict[str, InventoryEntry], natives: dict[str, tuple[list[str], list[str]]], executable: str,
) -> None:
    executable = resolve_inventory_path(files, executable)
    if executable not in natives:
        raise ProvenanceError("official native tool executable is not an Apple Silicon Mach-O binary")
    executable_rpaths = natives[executable][1]
    roots = {entry.value for path, entry in files.items() if path.startswith("/opt/homebrew/opt/") and entry.kind == "symlink"}
    if any(path.startswith(JAVA_ANCHOR + "/") for path in files):
        roots.add(JAVA_ANCHOR)
    for owner, (dependencies, rpaths) in natives.items():
        for dependency in dependencies:
            if dependency.startswith("/usr/lib/") or dependency.startswith("/System/Library/"):
                if _normal_path(dependency) != dependency:
                    raise ProvenanceError("native OS dependency path is not canonical")
                continue
            if dependency.startswith("@rpath/"):
                bases = [_expand_loader(path, owner, executable) for path in rpaths]
                bases += [_expand_loader(path, executable, executable) for path in executable_rpaths]
                candidates = [_normal_path(base + dependency.removeprefix("@rpath")) for base in bases]
            else:
                candidates = [_expand_loader(dependency, owner, executable)]
            matched = False
            for candidate in candidates:
                # WHY: 後方の真正な候補があっても、先行する未認証rpathから別libraryをloadできる。
                mapped = candidate
                seen_aliases: set[str] = set()
                for _ in range(64):
                    aliases = [key for key, value in files.items() if value.kind == "symlink"
                               and (mapped == key or mapped.startswith(key + "/"))]
                    if not aliases:
                        break
                    alias = max(aliases, key=len)
                    if alias in seen_aliases:
                        raise ProvenanceError("native dependency alias contains a cycle")
                    seen_aliases.add(alias)
                    mapped = files[alias].value + mapped[len(alias):]
                else:
                    raise ProvenanceError("native dependency alias exceeds its bound")
                if mapped.startswith("/usr/lib/") or mapped.startswith("/System/Library/"):
                    matched = True
                    continue
                if not any(mapped == root or mapped.startswith(root + "/") for root in roots):
                    raise ProvenanceError("native tool has an unclassified library search path")
                try:
                    resolved = resolve_inventory_path(files, candidate)
                except ProvenanceError:
                    continue
                if resolved in natives:
                    matched = True
            if not matched:
                raise ProvenanceError("native tool loads an unclassified external dependency")


def fetch_official_host_tool_proof(
    macos_version: str, *, fetch_json: Callable[[str], Any] | None = None,
    fetch_archive: Callable[[str], bytes] | None = None,
) -> HostToolProof:
    if not isinstance(macos_version, str) or re.fullmatch(r"(?:15|26)(?:\.\d+){0,2}", macos_version) is None:
        raise ProvenanceError("native tool evidence requires a supported macOS release")
    tag = "arm64_sequoia" if macos_version.split(".")[0] == "15" else "arm64_tahoe"
    read_json = fetch_json or fetch_official_json
    read_archive = fetch_archive or fetch_official_archive
    identities: dict[str, Any] = {"graphviz": [], "java": {}, "host_tcb": HOST_TCB_POLICY}
    files: dict[str, InventoryEntry] = {}
    natives: dict[str, tuple[list[str], list[str]]] = {}
    pending = ["graphviz"]
    selected: set[str] = set()
    runtime_versions: dict[str, set[str]] = {}
    budget = _Budget()
    while pending:
        name = pending.pop()
        if name in selected:
            continue
        selected.add(name)
        if len(selected) > MAX_FORMULAS:
            raise ProvenanceError("Graphviz runtime dependency closure exceeds its formula bound")
        metadata = read_json(FORMULA_API + _name(name) + ".json")
        identity, dependencies = _formula(metadata, name, tag)
        for item in metadata.get("runtime_dependencies") or []:
            revision = item.get("revision", 0)
            if type(revision) is not int or not 0 <= revision <= 100_000:
                raise ProvenanceError("Homebrew runtime dependency revision is unsupported")
            version = item["version"] + (f"_{revision}" if revision else "")
            runtime_versions.setdefault(item["full_name"], set()).add(version)
        pending.extend(dependency for dependency in dependencies if dependency not in selected)
        prefix = f"{name}/{identity['version']}"
        inventory, native = _archive_inventory(read_archive(identity["url"]), identity["sha256"], prefix, budget)
        inventory.setdefault("INSTALL_RECEIPT.json", InventoryEntry("installer-metadata", 0, ""))
        root = CELLAR + "/" + prefix
        for relative, entry in inventory.items():
            absolute = root + "/" + relative
            if absolute in files:
                raise ProvenanceError("official Graphviz closure has overlapping installations")
            files[absolute] = InventoryEntry(entry.kind, entry.mode, root + "/" + entry.value if entry.kind == "symlink" and entry.value != "." else root if entry.kind == "symlink" else entry.value)
        natives.update({root + "/" + relative: value for relative, value in native.items()})
        files[f"/opt/homebrew/opt/{name}"] = InventoryEntry("symlink", 0, root)
        identities["graphviz"].append(identity)
    identities["graphviz"].sort(key=lambda value: value["name"])
    selected_versions = {item["name"]: item["version"] for item in identities["graphviz"]}
    if any(versions != {selected_versions.get(name)} for name, versions in runtime_versions.items()):
        raise ProvenanceError("Homebrew runtime dependency versions do not match the official closure snapshot")
    graphviz = next(value for value in identities["graphviz"] if value["name"] == "graphviz")
    dot_path = f"{CELLAR}/graphviz/{graphviz['version']}/bin/dot"
    files["/opt/homebrew/bin/dot"] = InventoryEntry("symlink", 0, dot_path)
    _verify_native_closure(files, natives, dot_path)
    java_identity = _java_release(read_json(JAVA_RELEASE_API))
    archive = read_archive(java_identity["url"])
    # WHY: JDKのtop-level名はreleaseから決まり、任意のContents/Homeを拾う選択を避ける。
    prefix = java_identity["release"].removeprefix("jdk-")
    java_prefix = f"jdk-{prefix}"
    java_bundle, java_bundle_native = _archive_inventory(archive, java_identity["sha256"], java_prefix, budget)
    java_files: dict[str, InventoryEntry] = {}
    for relative, entry in java_bundle.items():
        if not relative.startswith("Contents/Home/"):
            continue
        if entry.kind == "symlink":
            if not entry.value.startswith("Contents/Home/"):
                raise ProvenanceError("official JDK home link escapes its runtime tree")
            entry = InventoryEntry(entry.kind, entry.mode, entry.value.removeprefix("Contents/Home/"))
        java_files[relative.removeprefix("Contents/Home/")] = entry
    java_native = {relative.removeprefix("Contents/Home/"): value for relative, value in java_bundle_native.items()
                   if relative.startswith("Contents/Home/")}
    anchored_files = {JAVA_ANCHOR + "/" + relative: InventoryEntry(entry.kind, entry.mode,
                      JAVA_ANCHOR + "/" + entry.value if entry.kind == "symlink" and entry.value != "." else JAVA_ANCHOR if entry.kind == "symlink" else entry.value)
                      for relative, entry in java_files.items()}
    anchored_native = {JAVA_ANCHOR + "/" + relative: value for relative, value in java_native.items()}
    if "lib/server/libjvm.dylib" not in java_files or "lib/modules" not in java_files:
        raise ProvenanceError("official Temurin archive does not contain the complete JNI runtime")
    _verify_native_closure(anchored_files, anchored_native, JAVA_ANCHOR + "/bin/java")
    identities["java"] = java_identity
    return HostToolProof(files, java_files, identities, dot_path)


def serialized_host_tool_proof(proof: HostToolProof) -> dict[str, Any]:
    return {"policy": TOOL_PROVENANCE_POLICY, "identities": proof.identities,
            "graphviz_inventory_sha256": _sha(_canonical(proof.graphviz_files)),
            "java_inventory_sha256": _sha(_canonical(proof.java_files)), "dot_path": proof.dot_path}


def _installed_digest(path: Path) -> str:
    result = hashlib.sha256()
    consumed = 0
    with path.open("rb") as source:
        while True:
            content = source.read(1024 * 1024)
            if not content:
                return result.hexdigest()
            consumed += len(content)
            if consumed > MAX_FILE_BYTES:
                raise ProvenanceError("installed host tool file exceeds its byte bound")
            result.update(content)


def _verify_entry(path: Path, entry: InventoryEntry, target: Path | None = None) -> None:
    try:
        metadata = path.lstat()
        if entry.kind == "file":
            if (not stat.S_ISREG(metadata.st_mode) or metadata.st_mode & 0o7000
                    or metadata.st_mode & 0o111 != entry.mode
                    or metadata.st_size > MAX_FILE_BYTES or _installed_digest(path) != entry.value):
                raise ProvenanceError("installed host tool differs from authenticated archive bytes")
        elif entry.kind == "symlink":
            if (not stat.S_ISLNK(metadata.st_mode) or metadata.st_mode & 0o7000
                    or target is None or path.resolve(strict=True) != target.resolve(strict=True)):
                raise ProvenanceError("installed host tool link differs from authenticated archive")
        elif entry.kind == "installer-metadata":
            if (not stat.S_ISREG(metadata.st_mode) or metadata.st_mode & 0o7111
                    or metadata.st_size > MAX_METADATA_BYTES):
                raise ProvenanceError("installer metadata is executable, special, or oversized")
        else:
            raise ProvenanceError("installed host tool inventory kind is unsupported")
    except OSError as exc:
        raise ProvenanceError("authenticated host tool file is unavailable") from exc


def _verify_tree(root: Path, expected: set[Path]) -> None:
    try:
        if root.is_symlink() or not root.is_dir():
            raise ProvenanceError("authenticated host tool installation root is unsupported")
        actual: set[Path] = set()
        pending = [root]
        count = 0
        while pending:
            directory = pending.pop()
            for child in directory.iterdir():
                count += 1
                if count > MAX_ENTRIES:
                    raise ProvenanceError("installed host tool tree exceeds its entry bound")
                metadata = child.lstat()
                if stat.S_ISDIR(metadata.st_mode):
                    if metadata.st_mode & 0o5000:
                        raise ProvenanceError("installed host tool directory has privileged mode")
                    pending.append(child)
                else:
                    actual.add(child)
        if actual != expected:
            raise ProvenanceError("installed host tool tree contains missing or unclassified files")
    except OSError as exc:
        raise ProvenanceError("authenticated host tool installation tree is unavailable") from exc


def verify_installed_host_tools(proof: HostToolProof, java_home: Path) -> None:
    # WHY: inventoryだけ一致しても親directoryのsymlinkで別treeを選べるため、実配置を先に固定する。
    try:
        java_home = java_home.resolve(strict=True)
        if not java_home.is_absolute() or not java_home.is_dir():
            raise ProvenanceError("selected Java home is unavailable")
        graphviz_roots = [Path(CELLAR) / item["name"] / item["version"] for item in proof.identities["graphviz"]]
        for root in graphviz_roots:
            if root.resolve(strict=True) != root:
                raise ProvenanceError("Graphviz installation ancestry contains an unsupported link")
            expected = {Path(path) for path in proof.graphviz_files if path.startswith(str(root) + "/")}
            _verify_tree(root, expected)
        _verify_tree(java_home, {java_home / path for path in proof.java_files})
        for raw_path, entry in proof.graphviz_files.items():
            _verify_entry(Path(raw_path), entry, Path(entry.value) if entry.kind == "symlink" else None)
        for relative, entry in proof.java_files.items():
            _verify_entry(java_home / relative, entry, java_home / entry.value if entry.kind == "symlink" else None)
    except (OSError, KeyError, TypeError) as exc:
        raise ProvenanceError("installed host tool provenance is incomplete") from exc

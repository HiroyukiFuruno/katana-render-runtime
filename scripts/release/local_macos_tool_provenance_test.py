from __future__ import annotations

import copy
import hashlib
import importlib.util
import io
import json
import struct
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location("local_macos_tool_provenance", Path(__file__).with_name("local_macos_tool_provenance.py"))
assert SPEC and SPEC.loader
PROVENANCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROVENANCE)


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def macho(dependencies: tuple[str, ...] = (), rpaths: tuple[str, ...] = ()) -> bytes:
    commands = []
    for command, paths, base in ((0xC, dependencies, 24), (0x8000001C, rpaths, 12)):
        for path in paths:
            content = path.encode() + b"\0"
            size = (base + len(content) + 7) // 8 * 8
            header = struct.pack("<III", command, size, base)
            if base == 24:
                header += b"\0" * 12
            commands.append(header + content + b"\0" * (size - base - len(content)))
    body = b"".join(commands)
    return struct.pack("<IIIIIIII", 0xFEEDFACF, 0x0100000C, 0, 2, len(commands), len(body), 0, 0) + body


def archive(files: dict[str, bytes | tuple[str, str]], *, extra: list[tarfile.TarInfo] | None = None) -> bytes:
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz", format=tarfile.PAX_FORMAT) as target:
        for path, value in files.items():
            entry = tarfile.TarInfo(path)
            entry.mode = 0o644 if path.endswith("INSTALL_RECEIPT.json") else 0o755
            if isinstance(value, tuple):
                entry.type = tarfile.SYMTYPE
                entry.linkname = value[1]
                target.addfile(entry)
            else:
                entry.size = len(value)
                target.addfile(entry, io.BytesIO(value))
        for entry in extra or []:
            target.addfile(entry, io.BytesIO(b"x" * entry.size) if entry.isfile() else None)
    return output.getvalue()


def formula(name: str, bottle: bytes, dependencies: list[str]) -> dict:
    sha = digest(bottle)
    return {
        "name": name, "full_name": name, "versions": {"stable": "1.0", "bottle": True}, "revision": 0,
        "dependencies": dependencies, "recommended_dependencies": [], "build_dependencies": ["compiler-only"],
        "bottle": {"stable": {"root_url": "https://ghcr.io/v2/homebrew/core", "files": {
            tag: {"cellar": PROVENANCE.CELLAR, "sha256": sha,
                  "url": f"https://ghcr.io/v2/homebrew/core/{name.replace('@', '/')}/blobs/sha256:{sha}"}
            for tag in ("arm64_sequoia", "arm64_tahoe")
        }}},
    }


class OfficialFixture:
    def __init__(self, *, external_dependency: str | None = None, graphviz_wrapper: bool = False) -> None:
        dependencies = (external_dependency or "/opt/homebrew/opt/cairo/lib/libcairo.2.dylib", "/usr/lib/libSystem.B.dylib")
        self.graphviz_contents = {
            "graphviz/1.0/bin/dot": b"#!/bin/sh\nexit 0\n" if graphviz_wrapper else macho(dependencies),
            "graphviz/1.0/lib/libgvc.6.dylib": macho(dependencies),
            "graphviz/1.0/lib/libgvc.dylib": ("symlink", "libgvc.6.dylib"),
            "graphviz/1.0/lib/graphviz/libgvplugin_core.6.dylib": macho(dependencies),
            "graphviz/1.0/lib/graphviz/config6a": b"libgvplugin_core.6.dylib core {}\n",
        }
        self.cairo_contents = {"cairo/1.0/lib/libcairo.2.dylib": macho(("/usr/lib/libSystem.B.dylib",))}
        self.java_contents = {
            "jdk-21.0.12.1+1/Contents/Info.plist": b"official bundle metadata",
            "jdk-21.0.12.1+1/Contents/Home/bin/java": macho(("@rpath/libjli.dylib",), ("@executable_path/../lib",)),
            "jdk-21.0.12.1+1/Contents/Home/lib/libjli.dylib": macho(("/usr/lib/libSystem.B.dylib",)),
            "jdk-21.0.12.1+1/Contents/Home/lib/server/libjvm.dylib": macho(("@loader_path/../libjli.dylib",)),
            "jdk-21.0.12.1+1/Contents/Home/lib/modules": b"authentic runtime modules",
            "jdk-21.0.12.1+1/Contents/Home/legal/java.base/LICENSE": b"authentic legal file",
            "jdk-21.0.12.1+1/Contents/Home/legal/java.logging/LICENSE": ("symlink", "../java.base/LICENSE"),
        }
        self.bottles = {"graphviz": archive(self.graphviz_contents), "cairo": archive(self.cairo_contents)}
        self.formulas = {"graphviz": formula("graphviz", self.bottles["graphviz"], ["cairo"]),
                         "cairo": formula("cairo", self.bottles["cairo"], [])}
        self.java_archive = archive(self.java_contents)
        self.java_url = "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.12.1%2B1/OpenJDK21U-jdk_aarch64_mac_hotspot_21.0.12.1_1.tar.gz"
        self.release = {"draft": False, "prerelease": False, "tag_name": "jdk-21.0.12.1+1", "assets": [
            {"name": "OpenJDK21U-jdk_aarch64_mac_hotspot_21.0.12.1_1.tar.gz", "size": len(self.java_archive),
             "browser_download_url": self.java_url, "digest": "sha256:" + digest(self.java_archive)},
        ]}
        self.requests: list[str] = []

    def json(self, url: str):
        self.requests.append(url)
        if url == PROVENANCE.JAVA_RELEASE_API:
            return self.release
        name = url.removeprefix(PROVENANCE.FORMULA_API).removesuffix(".json")
        return self.formulas[name]

    def download(self, url: str) -> bytes:
        if url == self.java_url:
            return self.java_archive
        for name, value in self.formulas.items():
            if url == value["bottle"]["stable"]["files"]["arm64_sequoia"]["url"]:
                return self.bottles[name]
        raise AssertionError(f"unexpected official archive: {url}")

    def proof(self, version: str = "15.7"):
        return PROVENANCE.fetch_official_host_tool_proof(version, fetch_json=self.json, fetch_archive=self.download)

    def install(self, base: Path):
        base = base.resolve()
        proof = self.proof()
        homebrew = base / "opt/homebrew"
        java_home = base / "temurin/Contents/Home"
        contents = {PROVENANCE.CELLAR + "/" + path: value for path, value in {**self.graphviz_contents, **self.cairo_contents}.items()}
        def remap(path: str) -> str:
            return str(homebrew) + path.removeprefix("/opt/homebrew") if path.startswith("/opt/homebrew") else path
        mapped = {remap(path): PROVENANCE.InventoryEntry(entry.kind, entry.mode, remap(entry.value) if entry.kind == "symlink" else entry.value)
                  for path, entry in proof.graphviz_files.items()}
        for original, entry in proof.graphviz_files.items():
            path = Path(remap(original))
            path.parent.mkdir(parents=True, exist_ok=True)
            if entry.kind == "file":
                path.write_bytes(contents[original])
                path.chmod(0o644 | entry.mode)
            elif entry.kind == "installer-metadata":
                path.write_bytes(b'{"poured_from_bottle":true,"time":123}')
                path.chmod(0o644)
            else:
                path.symlink_to(remap(entry.value))
        for relative, entry in proof.java_files.items():
            path = java_home / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if entry.kind == "file":
                path.write_bytes(self.java_contents["jdk-21.0.12.1+1/Contents/Home/" + relative])
                path.chmod(0o644 | entry.mode)
            else:
                path.symlink_to(java_home / entry.value)
        return PROVENANCE.HostToolProof(mapped, proof.java_files, proof.identities, remap(proof.dot_path)), java_home, homebrew / "Cellar"


class ToolProvenanceTest(unittest.TestCase):
    def test_pure_official_derivation_authenticates_native_dependency_and_jvm_tree(self) -> None:
        fixture = OfficialFixture()
        with patch.object(Path, "read_bytes", side_effect=AssertionError("remote derivation must not read local installations")), patch.object(
            Path, "lstat", side_effect=AssertionError("remote derivation must not inspect local installations")
        ):
            proof = fixture.proof()
        self.assertIn("/opt/homebrew/Cellar/cairo/1.0/lib/libcairo.2.dylib", proof.graphviz_files)
        self.assertIn("lib/server/libjvm.dylib", proof.java_files)
        self.assertIn("lib/modules", proof.java_files)
        self.assertEqual(PROVENANCE.resolve_inventory_path(proof.graphviz_files, "/opt/homebrew/bin/dot"), proof.dot_path)
        self.assertEqual(len(fixture.requests), 3)
        self.assertFalse(any("compiler-only" in value for value in fixture.requests))
        self.assertEqual(json.loads(json.dumps(PROVENANCE.serialized_host_tool_proof(proof)))["policy"], PROVENANCE.TOOL_PROVENANCE_POLICY)

    def test_current_official_formula_revision_and_versioned_namespace_are_supported(self) -> None:
        value = formula("openssl@3", b"fixture", [])
        value["revision"] = 2
        identity, _ = PROVENANCE._formula(value, "openssl@3", "arm64_sequoia")
        self.assertEqual(identity["version"], "1.0_2")
        self.assertIn("/openssl/3/", identity["url"])

    def test_supported_os_selects_native_bottle_and_unknown_os_declines(self) -> None:
        proof = OfficialFixture().proof("26.0")
        self.assertTrue(all(value["bottle_tag"] == "arm64_tahoe" for value in proof.identities["graphviz"]))
        for version in ("14.7", "27", "15;evil", "", None):
            with self.subTest(version=version), self.assertRaises(PROVENANCE.ProvenanceError):
                OfficialFixture().proof(version)

    def test_official_all_bottle_is_supported_but_linux_only_is_not(self) -> None:
        value = formula("certificates", b"official certificates", [])
        files = value["bottle"]["stable"]["files"]
        files["all"] = files.pop("arm64_sequoia")
        files.pop("arm64_tahoe")
        identity, _ = PROVENANCE._formula(value, "certificates", "arm64_sequoia")
        self.assertEqual(identity["bottle_tag"], "all")
        files["arm64_linux"] = files.pop("all")
        with self.assertRaises(PROVENANCE.ProvenanceError):
            PROVENANCE._formula(value, "certificates", "arm64_sequoia")

    def test_metadata_url_digest_name_and_runtime_classes_fail_closed(self) -> None:
        for field, replacement in (("full_name", "foreign/graphviz"), ("dependencies", ["../../foreign"]),
                                   ("recommended_dependencies", ["cairo"]), ("runtime_dependencies", "unknown")):
            fixture = OfficialFixture()
            fixture.formulas["graphviz"][field] = replacement
            with self.subTest(field=field), self.assertRaises(PROVENANCE.ProvenanceError):
                fixture.proof()

    def test_explicit_runtime_metadata_must_match_recursive_selected_versions(self) -> None:
        fixture = OfficialFixture()
        fixture.formulas["graphviz"]["runtime_dependencies"] = [
            {"full_name": "cairo", "version": "1.0", "revision": 0, "declared_directly": True},
        ]
        fixture.proof()
        fixture.formulas["graphviz"]["runtime_dependencies"][0]["version"] = "0.9"
        with self.assertRaises(PROVENANCE.ProvenanceError):
            fixture.proof()
        for field, replacement in (("url", "https://evil.example/archive"), ("sha256", "f" * 63), ("cellar", "/untrusted/Cellar")):
            fixture = OfficialFixture()
            fixture.formulas["graphviz"]["bottle"]["stable"]["files"]["arm64_sequoia"][field] = replacement
            with self.subTest(field=field), self.assertRaises(PROVENANCE.ProvenanceError):
                fixture.proof()

    def test_java_release_identity_and_asset_ambiguity_fail_closed(self) -> None:
        variants = [lambda value: value.update(draft=True), lambda value: value.update(prerelease=True),
                    lambda value: value.update(tag_name="jdk-17.0.1+1"),
                    lambda value: value["assets"].append(copy.deepcopy(value["assets"][0])),
                    lambda value: value["assets"][0].update(digest="sha256:" + "0" * 64),
                    lambda value: value["assets"][0].update(browser_download_url="https://github.com/foreign/runtime/archive.tar.gz")]
        for change in variants:
            fixture = OfficialFixture()
            change(fixture.release)
            with self.assertRaises(PROVENANCE.ProvenanceError):
                fixture.proof()

    def test_external_native_library_and_official_wrapper_are_rejected(self) -> None:
        for fixture in (OfficialFixture(external_dependency="/tmp/libfake.dylib"), OfficialFixture(graphviz_wrapper=True),
                        OfficialFixture(external_dependency="/usr/lib/../../tmp/libfake.dylib")):
            with self.assertRaises(PROVENANCE.ProvenanceError):
                fixture.proof()

    def test_earlier_unclassified_rpath_cannot_be_hidden_by_later_authentic_match(self) -> None:
        fixture = OfficialFixture()
        fixture.graphviz_contents["graphviz/1.0/bin/dot"] = macho(("@rpath/libcairo.2.dylib",),
            ("/tmp/hostile", "/opt/homebrew/opt/cairo/lib"))
        fixture.bottles["graphviz"] = archive(fixture.graphviz_contents)
        fixture.formulas["graphviz"] = formula("graphviz", fixture.bottles["graphviz"], ["cairo"])
        with self.assertRaises(PROVENANCE.ProvenanceError):
            fixture.proof()

    def test_known_absent_in_tree_rpath_then_authentic_match_is_supported(self) -> None:
        fixture = OfficialFixture()
        fixture.graphviz_contents["graphviz/1.0/bin/dot"] = macho(("@rpath/libcairo.2.dylib",),
            ("@executable_path/../lib", "/opt/homebrew/opt/cairo/lib"))
        fixture.bottles["graphviz"] = archive(fixture.graphviz_contents)
        fixture.formulas["graphviz"] = formula("graphviz", fixture.bottles["graphviz"], ["cairo"])
        self.assertIn("graphviz", [value["name"] for value in fixture.proof().identities["graphviz"]])

    def test_java_class_magic_is_not_treated_as_universal_native_code(self) -> None:
        self.assertIsNone(PROVENANCE._native_dependencies(bytes.fromhex("cafebabe000000410001")))
        with self.assertRaises(PROVENANCE.ProvenanceError):
            PROVENANCE._native_dependencies(bytes.fromhex("cafebabe0000000200000000"))

    def test_installer_receipt_can_change_but_remains_nonexecuting_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            proof, java_home, cellar = OfficialFixture().install(Path(temporary))
            receipt = cellar / "graphviz/1.0/INSTALL_RECEIPT.json"
            receipt.write_text('{"poured_from_bottle":false,"time":999}', encoding="utf-8")
            with patch.object(PROVENANCE, "CELLAR", str(cellar)):
                PROVENANCE.verify_installed_host_tools(proof, java_home)
                receipt.chmod(0o755)
                with self.assertRaises(PROVENANCE.ProvenanceError):
                    PROVENANCE.verify_installed_host_tools(proof, java_home)

    def test_valid_installed_inventory_and_symlinks_pass_then_tampering_fails(self) -> None:
        mutations = ("dot", "dependency", "plugin", "config", "jvm", "modules", "extra", "missing", "opt-alias", "mode")
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                proof, java_home, cellar = OfficialFixture().install(Path(temporary))
                with patch.object(PROVENANCE, "CELLAR", str(cellar)):
                    PROVENANCE.verify_installed_host_tools(proof, java_home)
                    targets = {"dot": Path(proof.dot_path), "dependency": cellar / "cairo/1.0/lib/libcairo.2.dylib",
                               "plugin": cellar / "graphviz/1.0/lib/graphviz/libgvplugin_core.6.dylib",
                               "config": cellar / "graphviz/1.0/lib/graphviz/config6a",
                               "jvm": java_home / "lib/server/libjvm.dylib", "modules": java_home / "lib/modules"}
                    if mutation in targets:
                        targets[mutation].write_bytes(b"#!/bin/sh\nexit 0\n")
                    elif mutation == "extra":
                        (java_home / "lib/hostile.dylib").write_bytes(macho())
                    elif mutation == "missing":
                        (java_home / "lib/modules").unlink()
                    elif mutation == "opt-alias":
                        alias = cellar.parent / "opt/cairo"
                        alias.unlink()
                        alias.symlink_to(cellar / "graphviz/1.0")
                    else:
                        Path(proof.dot_path).chmod(0o644)
                    with self.assertRaises(PROVENANCE.ProvenanceError):
                        PROVENANCE.verify_installed_host_tools(proof, java_home)

    def test_archive_digest_checked_before_parsing_and_inventory_bounds(self) -> None:
        with self.assertRaises(PROVENANCE.ProvenanceError):
            PROVENANCE._archive_inventory(b"not tar", "0" * 64, "graphviz/1.0", PROVENANCE._Budget())
        blob = archive({"graphviz/1.0/file": b"12345", "graphviz/1.0/second": b"x"})
        for limit in ("MAX_ENTRIES", "MAX_CONTENT_BYTES", "MAX_UNCOMPRESSED_ARCHIVE_BYTES", "MAX_TOTAL_ARCHIVE_BYTES", "MAX_FILE_BYTES"):
            with self.subTest(limit=limit), patch.object(PROVENANCE, limit, 1), self.assertRaises(PROVENANCE.ProvenanceError):
                PROVENANCE._archive_inventory(blob, digest(blob), "graphviz/1.0", PROVENANCE._Budget())

    def test_authenticated_jdk_setgid_directories_are_metadata_not_privileged_files(self) -> None:
        directory = tarfile.TarInfo("jdk-21.0.12.1+1/Contents/Home/bin")
        directory.type = tarfile.DIRTYPE
        directory.mode = 0o2755
        blob = archive({"jdk-21.0.12.1+1/Contents/Home/bin/java": macho()}, extra=[directory])
        inventory, _ = PROVENANCE._archive_inventory(blob, digest(blob), "jdk-21.0.12.1+1", PROVENANCE._Budget())
        self.assertIn("Contents/Home/bin/java", inventory)
        with tempfile.TemporaryDirectory() as temporary:
            proof, java_home, cellar = OfficialFixture().install(Path(temporary))
            (java_home / "bin").chmod(0o2755)
            with patch.object(PROVENANCE, "CELLAR", str(cellar)):
                PROVENANCE.verify_installed_host_tools(proof, java_home)
                (java_home / "bin/java").chmod(0o2755)
                with self.assertRaises(PROVENANCE.ProvenanceError):
                    PROVENANCE.verify_installed_host_tools(proof, java_home)
        privileged = tarfile.TarInfo("jdk-21.0.12.1+1/Contents/Home/privileged")
        privileged.mode = 0o2755
        privileged.size = 1
        blob = archive({"jdk-21.0.12.1+1/Contents/Home/bin/java": macho()}, extra=[privileged])
        with self.assertRaises(PROVENANCE.ProvenanceError):
            PROVENANCE._archive_inventory(blob, digest(blob), "jdk-21.0.12.1+1", PROVENANCE._Budget())

    def test_archive_unsafe_paths_links_duplicates_and_special_entries_decline(self) -> None:
        blobs = [archive({"../escaped": b"x"}), archive({"/absolute": b"x"}), archive({"graphviz/1.0/../escape": b"x"}),
                 archive({"foreign/1.0/file": b"x"}),
                 archive({"graphviz/1.0/link": ("symlink", "../../../escape")}),
                 archive({"graphviz/1.0/link": ("symlink", "/tmp/escape")})]
        for kind in (tarfile.LNKTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE):
            entry = tarfile.TarInfo("graphviz/1.0/special")
            entry.type = kind
            entry.linkname = "graphviz/1.0/file"
            blobs.append(archive({"graphviz/1.0/file": b"x"}, extra=[entry]))
        duplicate = tarfile.TarInfo("graphviz/1.0/file")
        duplicate.size = 1
        blobs.append(archive({"graphviz/1.0/file": b"x"}, extra=[duplicate]))
        for blob in blobs:
            with self.assertRaises(PROVENANCE.ProvenanceError):
                PROVENANCE._archive_inventory(blob, digest(blob), "graphviz/1.0", PROVENANCE._Budget())

    def test_missing_official_artifact_or_metadata_declines_without_executing_installer(self) -> None:
        fixture = OfficialFixture()
        fixture.formulas["graphviz"]["bottle"]["stable"]["files"].pop("arm64_sequoia")
        with self.assertRaises(PROVENANCE.ProvenanceError):
            fixture.proof()
        with self.assertRaises(PROVENANCE.ProvenanceError):
            PROVENANCE.fetch_official_host_tool_proof("15.7", fetch_json=lambda _url: None,
                                                     fetch_archive=lambda _url: self.fail("must not download"))

    def test_duplicate_json_keys_are_rejected(self) -> None:
        with self.assertRaises(PROVENANCE.ProvenanceError):
            json.loads('{"digest":"good","digest":"bad"}', object_pairs_hook=PROVENANCE._unique_object)


if __name__ == "__main__":
    unittest.main()

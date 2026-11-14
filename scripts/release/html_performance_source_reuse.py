"""測定済みsourceと文書のみ異なる現sourceの対応を検証する。"""
from __future__ import annotations

import hashlib
import re
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

ALLOWED = frozenset({
    "docs/performance/html-performance-history.json",
    "docs/performance/README.md",
    "docs/quality-gate-improvement-tasks.md",
})
TEST_ROOT = "crates/katana-render-runtime/src/"
TEST_PATHS = frozenset(TEST_ROOT + name for name in (
    "svg_engine_filter_parser_boundary_tests.rs", "svg_engine_parser_boundary_tests.rs",
    "svg_engine_text_layout_spacing_tests.rs",
))
TEST_PARENT = TEST_ROOT + "svg_engine.rs"
TEST_LOGGER = """
#[cfg(test)]
fn init_test_logger() {
    static LOGGER: std::sync::Once = std::sync::Once::new();
    LOGGER.call_once(|| {
        if let Err(error) = env_logger::Builder::new()
            .is_test(true)
            .filter_level(log::LevelFilter::Warn)
            .try_init()
        {
            assert!(log::log_enabled!(log::Level::Warn), "{error}");
        }
    });
}
"""
# WHY: 接続の構文を既確認の特定sourceに固定し、文字列の出現では認定しない。
TEST_PARENT_SHA = frozenset({
    "7531f7db0247100b036f30e7f8215c29a9773805332e919ea8c8a8f3854009b1",
    "c18945e539f8e6176c435ae0104b96d9b964b544d63afcdbff0d96fbb2c4bd39",
})
TEST_ANCHOR_SHA = {
    TEST_ROOT + "lib.rs": "3099318935f89f3ee844f496cf69a2e5595db02233288a3236f60bf6945f7d3f",
    TEST_ROOT + "svg_engine_filter_parser_model_tests.rs": "58239e065fb9db3c7f96daa84da10297e45441e6c5ed30a845458ecd7f4368fa",
    TEST_ROOT + "svg_engine_text_model_tests.rs": "acebe5d6cbac8e9fdda49d5fc9035a11bcc47628c677a4255deccf8c95c279e3",
    TEST_ROOT + "svg_engine_text_layout_matrix_tests.rs": "1904c2b7bfc053712b83988c66432463e2c9581370daea0641925c4e35219be6",
    "crates/katana-render-runtime/vendor/svg-engine/usvg/lib.rs": "15fabe610787a1ba7905b45fe2d9c07f8fa0c2dbf5b8204f719fea3c0c48bed0",
    "crates/katana-render-runtime/vendor/svg-engine/usvg/tree/mod.rs": "f052759059bcd1b1b09a0a94c3046a6c48b1c157d7373cb318f0be9c9177bb50",
    "crates/katana-render-runtime/vendor/svg-engine/usvg/tree/text.rs": "7b48f425e37fd5fa71d336f85ee0be6df0b1222143f08b18c57c10a3f2b5f307",
}
Records = dict[str, tuple[str, int]]
GitBytes = Callable[[Sequence[str]], bytes]
GitText = Callable[[Sequence[str]], str]

RELEASE_ONLY_PROGRAMS = frozenset({
    "scripts/release/verify-release-target.py",
    "scripts/release/verify_release_target_test.py",
})

# 計測済みHEADから、計測プログラム自体に触れない評価移行だけを認証する。
ZERO_SCROLL_MEASURED_HEAD = "6e627a410f03ae014578a4c6c572b326d0c8edee"
ZERO_SCROLL_REVIEWED_FILES: dict[str, tuple[str, int]] = {
    "Justfile": ("0425ddbc52bc8d3e92d18bff2efa31c2742b9ad7fa73d7b48eb3c147074aec7e", 87249),
    "scripts/release/html_performance_memory_evidence.py": ("c6bad54819e93485e8c39e942c7653d654359ff5ca36472be86d67ec4ac1ef90", 6209),
    "scripts/release/html_performance_memory_evidence_test.py": ("64c8240bc0f1d372c20423833fba70605622164640438fbd33a00d3d7d1d00a7", 10373),
    "scripts/release/html_performance_memory_policy.py": ("ce2a5ce213f31d3993182bc78b04e647baf83fd95afae67cfd3021fbf4cbd3ac", 7772),
    "scripts/release/html_performance_promotion.py": ("0e94b79d89cbcf77986acbc6804793360d145e0ac7825c98d88229878944b612", 24136),
    "scripts/release/html_performance_promotion_test.py": ("b8b05eff50b0494769b6bcc364c5dbae17012530ee12a197cfee6d40f332b53c", 37193),
    "scripts/release/html_performance_reference.py": ("72c2b3bdd0c4a5082372ccb5eab2e2aac54f0baceb48e270f1acff30d8de4c66", 27632),
    "scripts/release/html_performance_reference_test.py": ("8a1e0b9e4e58e37cf23972af3da2d09ee57dbd71334718533f69f688a4918617", 30296),
    "scripts/release/html_performance_remote_admission.py": ("b991746d218c400600319eb04e554d7e9ce9649ccdc4513867786d59065d64e0", 3636),
    "scripts/release/html_performance_retention_collection.py": ("3f9d954237d9aa2442323c1ab768988dd13d81a46175549c9590dec94a17da0e", 2668),
    "scripts/release/html_performance_source_reuse_cli.py": ("a5f252f573074a16b4edb77682911889f3b3c883de9c851e47aab88cc469cf14", 12033),
    "scripts/release/html_performance_source_reuse_cli_test.py": ("dc20f190b59dca037fb87dc56038dca5805a95c0744713dd5891ed2fd044e784", 18868),
    "scripts/release/html_performance_source_reuse_test.py": ("12a31560e7851134f0cebb34fbf3f2a93c35aa83f07b378c08775da2a6eeda7a", 71986),
    "scripts/release/local_html_performance_evidence.py": ("5cce6fb5e8eba67965e10ba1be82a336f5a4fc298a1df52425d7fc6f2c1d71f3", 17553),
    "scripts/release/local_html_performance_evidence_test.py": ("e44bb5a569b64022bfc9e2c7ccb567efd44cabc16fe70cc3c488d3c07b04c721", 18654),
    "scripts/html/performance-history.ts": ("f52e9655c15905b7dedd1854bb3a4f1c955bbe2c175b27b70320264382249a8e", 7650),
    "scripts/html/performance-history_test.ts": ("b1d4b4513e4d70a65249b31a4dcaf422a58bf6a0241ff53015ee883671e418f8", 13609),
    "scripts/release/produce_html_performance_evidence.py": ("3d5f1ea31349e7633771dd25e7c9e89659323423cf5f8ccd84406fb8468df577", 274605),
    "scripts/release/produce_html_performance_evidence_test.py": ("cd522b01503fb9a32ec3ce99882207293f4d13f23b526fc2ff755217b68d116b", 330386),
    "scripts/release/verify_html_performance_evidence.py": ("e2c1d411847fb1b908233a1dadfc036d1b4ea83c6c219e403ae737f52b7fddd8", 280925),
    "scripts/release/verify_html_performance_evidence_test.py": ("87ba739d12ee9d6a847fef93d8c4de53184a3d341924f542ffa45f8c73ceaf18", 316406),
}
ZERO_SCROLL_SELF = "scripts/release/html_performance_source_reuse.py"


def is_zero_scroll_transition_program(source: bytes) -> bool:
    return (b"ZERO_SCROLL_MEASURED_HEAD" in source
            and reviewed_same_binary_self(source))


def _zero_scroll_policy_changes(
    measured_head: str, current: Records, measured: Records,
    old_modes: Mapping[str, str], new_modes: Mapping[str, str],
) -> set[str]:
    if measured_head != ZERO_SCROLL_MEASURED_HEAD or not ZERO_SCROLL_REVIEWED_FILES:
        return set()
    approved = set(ZERO_SCROLL_REVIEWED_FILES) | {ZERO_SCROLL_SELF}
    changed = {path for path in set(current) | set(measured) if measured.get(path) != current.get(path)}
    if changed - ALLOWED - RELEASE_ONLY_PROGRAMS != approved:
        return set()
    if any(old_modes.get(path) != "100644" or new_modes.get(path) != "100644"
           for path in approved):
        return set()
    body = Path(__file__).read_bytes()
    self_record = (hashlib.sha256(body).hexdigest(), len(body))
    if (current.get(ZERO_SCROLL_SELF) != self_record
            or any(current.get(path) != value for path, value in ZERO_SCROLL_REVIEWED_FILES.items())):
        return set()
    release_changes = changed & RELEASE_ONLY_PROGRAMS
    if any(old_modes.get(path) != "100644" or new_modes.get(path) != "100644"
           for path in release_changes):
        return set()
    return approved | release_changes

# WHY: 固定epoch後の検証専用cohortを実Git blob単位で限定する。
VALIDATION_COHORT_ADDITIONS: dict[str, tuple[str, int]] = {
    "scripts/drawio/candidate-svg-rasterize.ts": ("22b874614043742c806cbdb77862f37212480e4201eb0aad4a940cfba74bedda", 3621),
    "scripts/drawio/full_compare_inventory_test.ts": ("042093a5ccdea86f96d1eb3a6d732f0d6651cf63bb8f5466960c169eb9375215", 5165),
    "scripts/drawio/svg-capture.ts": ("b2fc26287b3d4a336db4497cb3f54039f9168295f3aff40f02dad5bc63868a79", 4476),
    "scripts/drawio/svg_capture_browser_test.ts": ("613745c16e650b4031477ba20aaf71cdf62b3b40385f504b12eb99a95f2b14d1", 1809),
    "scripts/drawio/svg_capture_test.ts": ("b706b5f2f43b71a6b14c4958646eea6e3c38afad8b0bc9f71cfbe2b391e479fe", 1532),
    "scripts/release/html_performance_same_binary_diagnostic.py": ("0f6a3ce678431eeb59bc021b390c5e71b0fb3005a5e33538fccc92b3706c6a12", 10533),
    "scripts/release/html_performance_same_binary_diagnostic_test.py": ("5cbeccc03f1b5093ad5c0ac992e17203071d9bd831b98decea602b95188bef01", 1722),
}
VALIDATION_COHORT_CHANGES: dict[str, tuple[tuple[str, int], tuple[str, int]]] = {
    "Justfile": (("5c44338aa86b1f711534e7aff2dc77626e2632f790a73ed90ed8af9da2bf7067", 84265), ("39d45b22a2f5f6d60ca4ee02cc1c8054de86f6a104b67fd4f6765039b3160dc9", 85579)),
    "scripts/drawio/official-renderer.ts": (("17ed1965601971df914cc9b24ea40bbc9562dc7d416acc27e32d8088136c8f2a", 7551), ("ae2d05f5df65ebb4bb07c684938dc9341bbedd670a50a51acf185d701853cea2", 5177)),
    "scripts/quality/owned-validation-contract.ts": (("c3ec88e882c759907f8f2ce69e9556a4fcdd94cdba6c45044e07fd745b13d33d", 218924), ("bea812c143b520f502c1d02840ad3d8d0233ae4016856adf5268df5cdd21e34b", 220172)),
    "scripts/quality/owned-validation-contract_test.ts": (("4b5c0fe741e31c8b10d366804f957e63211e0b0e7ecfff782aee57d2fd424a4e", 256387), ("d3a1199e399a20562d90b0e5e614e1b4491319b5073a91e4970570fe2deaedc3", 257191)),
    "scripts/release/html_performance_host_load_validation.py": (("80b59b6d1fb10ba7953e2f598bed2928483a8ee50aea09ae1992796f6239b87a", 10040), ("50c1fd0439f46472bb789865ecfd83714f6feaaaa8c882521880dab6710b3659", 10112)),
    "scripts/release/produce_html_performance_evidence.py": (("bffd5b7781ea1650185d2756a90c4c7350d7708726d7482cf6fff397c4b14aa8", 252854), ("80ded11e1b2984635cd455aa55dd926eefbbd03759d8c9eccd719dc16bfd7d45", 254521)),
    "scripts/release/produce_html_performance_evidence_test.py": (("7ff9328bebff71189ca1c32719923d50688665b88e284f0a2c4d16ff4c216cad", 292734), ("ea9484f7714c5279d7f8880fb4a499e4fd24fb8b7fa7818b227dd5075cdf9cc9", 295024)),
}


def validation_cohort_change(path: str, old: tuple[str, int] | None,
                             new: tuple[str, int] | None, old_mode: str | None,
                             new_mode: str | None) -> bool:
    if path in VALIDATION_COHORT_ADDITIONS:
        if path == "scripts/drawio/candidate-svg-rasterize.ts":
            formatted = ("96244e3710fd8b69307ad6da002cc67487bc6f55f43ed87d87eee7211e3ffbad", 3651)
            original = VALIDATION_COHORT_ADDITIONS[path]
            return ((old is None and old_mode is None and new in (original, formatted)
                     or old == original and old_mode == "100644" and new == formatted)
                    and new_mode == "100644")
        return (old is None and old_mode is None and new == VALIDATION_COHORT_ADDITIONS[path]
                and new_mode == "100644")
    just_launcher_changes = {
        "Justfile": (("1ddc7e15691a169667301c09896879df62142bf264e7a6b3c18b39a206c42b22", 85605),
            ("b0b8129c75f5f2da37b1fca70625a4d123c4185a7928fddea65036810097a0da", 85756)),
        "scripts/html/harness-contract-graph.ts": (("a514eac95f19c08d01eff4a65181e9079a3646f1e4d88a1b15946e375f8a9291", 197561),
            ("9f7e80b6007edf1b1d2c32e43478805779ca80a1d698732726e72a56bdabb86b", 197904)),
        "scripts/html/harness-contract_test.ts": (("731937522cdeb18769b038f760aef65d7b2bd10dbb29698385753dabefd1442d", 264923),
            ("6ee0ee3a528be4e1300498356964bd8226499256bba5f71a1fa4c9c3e4dd6269", 266949)),
        "scripts/mermaid/native-cli-managed-build-input-test-fixture.ts": (("071ed9ccaeb01df5d409fb61d01e98c9582897604b69596df222d872a750c2b4", 12580),
            ("5e95a724320af262f12babb8cd6886e529dfe222cd66e63cdbd5cfc9c9ef9f7b", 13630)),
        "scripts/mermaid/native-cli-managed-build-schema.ts": (("c8d92904803968c542a75ae9c520aaa948dd5e1b6d3dde7c54d08667bd44d02f", 7835),
            ("562f53afb924ca81acb367eed22bb441680aa336a7e15058a163dbba47d67d6e", 7982)),
        "scripts/mermaid/native-cli-managed-build-test-fixture.ts": (("37600a6eb713c062dde2521a23652f0e714e649222a1e13285de842b421a778a", 15036),
            ("db45ddd958784813c84f937630088e3b2c9b524122d3f40be01c37d14c739358", 16112)),
        "scripts/mermaid/native-cli-managed-build.ts": (("fc37d9ce83950f0d5596ca9e5b7f68ec14b83aec48c616e5de735130acb799c9", 11693),
            ("272e649de81bc2da8e6e4576ca32d249d4da1dff813a49020f1e3d68f551724b", 13999)),
        "scripts/mermaid/native-cli-managed-cargo.ts": (("1b78df712dd3bf87ccdebfd5faf8bece9abc0499a3af66a10e6d7394066ed74e", 20334),
            ("438b7e29486622020d61c368ea9ea29a67f8182be41792ef10748824bfa7ca50", 20358)),
        "scripts/mermaid/native-cli-managed-input-schema.ts": (("915e8bef83d7134a7e8df4ba4e0bb822d17735fd99d82bf67a169678d35dad8a", 4642),
            ("7c38a0d21d23a20fa91e5ab6ce4b62526519b3f51f3810be2765ba47c44660f1", 5613)),
        "scripts/mermaid/native-cli-managed-inputs.ts": (("b4cfeb9e28d0883081aef021eaf60dce60fcdef832c6af280d65b9911d342cd8", 7812),
            ("2e95e77675af9f26f2f53fd2c2a63f1b9e296fd42c7854666304627a1d82f882", 9205)),
        "scripts/mermaid/native_cli_build_bindings.py": (("4cc205fcc4869aead7ee63bf6bb911ad1f23858082f8787c390155dc9092b18b", 15900),
            ("997d9114d2f6faa3e753193d8b7fae04a57b8bb84146fde6e769692475fda3c3", 19203)),
        "scripts/mermaid/native_cli_build_producer_test.py": (("905d0f05094c416e684d344a1888deb107851651ec41b3ee4ce747940e3a62a5", 37118),
            ("8ab429d6b676153745f5a3e2b75a43b3f0171e7a68b721caae84ce72c1cb83d6", 40840)),
        "scripts/mermaid/native_cli_managed_build_reader_test.ts": (("11379abd45fdf80c7bdb487998cd8e51866620b8230d1f37cdd821530504d288", 33628),
            ("2434d0a53edc15f013636b3bb5a66b6cf7769c2e2a9253470849bb01344156da", 36985)),
        "scripts/mermaid/produce_native_cli_build_receipt.py": (("c5e346bb8bd2a36af6d36d6162555e377487c8f2a0134edd158dedc870f05743", 20268),
            ("6e9be36f90e0209b2fea288d9dc4462ef77cb6777c3e3df5b36e6bc7f8c5dee4", 20454)),
        "scripts/runtime-bundles/runtime-rust-facts-host-freshness_test.ts": (("4670b3578bd3d564e11d02bb4f514339ef990866c9e18fa8bd99d9c9ac99660f", 9119),
            ("817542ee4958eea7663550e11eef7cc757cccedf5355a217cf44cbeaaee579a0", 12068)),
        "scripts/runtime-bundles/runtime-rust-facts-host-inputs.ts": (("b053cfa3fb85a4de1bf761bd7dca39e36028bec15aa7a319ddb25340424bed32", 7599),
            ("b3b2d25b69b7df09eecf41d196a278025c435251eb66966d64e2937d2bc16bab", 9334)),
        "scripts/runtime-bundles/runtime-rust-facts-host-protocol.ts": (("ffe16c0ef5e50ec1b0573adb228f3a8151df2eeb5c0c890df6918533e359ce81", 14567),
            ("6fc68ddc1487baa396454c263f3a6de79fe759550fc0be356dfb1288d18b6aed", 14811)),
        "scripts/runtime-bundles/runtime-rust-facts-host.ts": (("a47f6005ab71db0027b888683973fa0c5fba0620eb209b7bff813381e4c31e08", 15040),
            ("ce45d76d1b8159cef32a671b5572a27fb7304bdd08904984903e7a6e57e0e8ba", 15585)),
        "scripts/runtime-bundles/runtime-rust-facts-host_test.ts": (("4837857c9dfe95b350d65592bd11d6117a5891f37a877927adbc88f5b44bdf22", 20282),
            ("edbac16de7851e51ceed681f4c2723874641ec557679dc03f06e77c0aa455000", 20436)),
    }
    if path in just_launcher_changes and (old, new) == just_launcher_changes[path]:
        return old_mode == new_mode == "100644"
    pair = VALIDATION_COHORT_CHANGES.get(path)
    if pair is not None:
        followup = {
            "scripts/release/produce_html_performance_evidence.py":
                ("5b5c62c6ec147f0ed9a541c6eb9d07a883cb3ad90d75eb71bc5b6d8f7750e08c", 254729),
            "scripts/release/html_performance_host_load_validation.py":
                ("81ca438ccce28208836dd811438ce81ef27961fe289051110750d1403f2069c5", 10184),
        }
        if old == pair[0] and new is not None and (new == pair[1] or new == followup.get(path)):
            return old_mode == new_mode == "100644"
    if path == "scripts/release/html_performance_source_reuse.py":
        original = ("6f679815a6417ed21e9dceddd9fa201e39a4ab7499288e5b49a4181f9ecf18ec", 24436)
        data = Path(__file__).read_bytes()
        live = (hashlib.sha256(data).hexdigest(), len(data))
        intermediate = ("8c3503e588e2e7ab93770a86c4e1ffb0c10ea463e2d98c808d2e6fef1a6895bd", 32039)
        reviewed = ("d3d6a94ad5cb0465a46cf0b98fb74a4fe9cb46c5a9ac5a72ca9fa4824e762015", 34030)
        retained = ("e11014eb370b864d812928235c7b033fca37acb8f2e0d8341bfd86c55c2672b2", 35728)
        latest_reviewed = ("37c6a3b87c0d5367d2144b8850f9cd7b97294e92fe32886fbb408768033d3ad2", 37269)
        budget_predecessor = ("0bde8075042a47d5601044e860d997f49e8aaf85a8b67a1c6ea862e37df90844", 40090)
        format_predecessor = ("10225a74072a6d30b58ca01d3a6e93f0797d75bc1b700b963c725bc4192a8384", 41784)
        promotion_predecessor = ("4a57d835945ea1b095b039fa565f80fbc01ee7e5d231bad1c7c560c53d94d6b0", 44350)
        just_predecessor = ("d92d9865cd7e501fccf45202d3eaee6df795f2f0d27bf0cc9d6d284ca5aa4099", 45823)
        history = (intermediate, reviewed, retained, latest_reviewed, budget_predecessor,
                   format_predecessor, promotion_predecessor, just_predecessor)
        if old in history:
            base = ("85d9ff1f649fcef4b85cdd7634b4ce4c711a4e567d7203eecbea33793954b4eb", 74484)
            return (new == live or new == base or new in history[history.index(old) + 1:]) and old_mode == new_mode == "100644"
        prior = ("7d75f7a2642103908ebaaf24ccffc175b0ff61e8bbb020fea9f9b0d0e4648dfe", 24917)
        if old != original or old_mode != "100644" or new_mode != "100644":
            return False
        if new in (prior, *history):
            return True
        if new != live:
            return False
        import subprocess
        baseline = subprocess.run(
            ["git", "--no-replace-objects", "-C", str(Path(__file__).resolve().parents[2]), "show",
             "b9cf37535ba374b62fee4e889f9c58f58f1ab8dc:" + path],
            check=True, capture_output=True,
        ).stdout
        return known_auth_program_change(path, baseline, data)
    fact_cache_changes = {
        "scripts/release/verify_html_performance_evidence.py": (
            ("04dc3d5e4d44f239c4bf1e664273d8ad0b4fcd22bb50591bed2535b6d36c9aef", 250526),
            ("65beb54c881365eafb607d9ced7f345f6fdc63221386bd16dc403e3a8b835fd2", 250579)),
        "scripts/release/verify_html_performance_evidence_test.py": (
            ("30f00fa2d26a68c4ed95bac4fb7624864fa2588914da65abf606c91fefbfff97", 255806),
            ("aa3211d3807addbacad8290e5724f46302f9a72451a96732b3cf070fe241c899", 256885)),
        "scripts/release/html_performance_source_reuse_test.py": (
            ("49cc989141f481b7d9e522265d4ebd3c70e7f6ac4f2b98ca36ec187b3a0a3351", 22385),
            ("5000e614d0690eaa50a1ffc0c3b1089c9fcf3b8ca13a71ea4bd8c3ea55fd128d", 25819)),
    }
    reviewed_program_changes = {
        "scripts/release/verify_html_performance_evidence.py": (
            frozenset({("630cb08266eb1b24b249bf295b3591cbe87da9529e6f770ce600715ad93ab656", 249838),
                       fact_cache_changes["scripts/release/verify_html_performance_evidence.py"][0],
                       fact_cache_changes["scripts/release/verify_html_performance_evidence.py"][1]}),
            ("610b1005cca73b8b3a8242e6b3219db65d0526225ae7cc5fb56032bcd8b0f151", 252718)),
        "scripts/release/verify_html_performance_evidence_test.py": (
            frozenset({("ee5f14f317dc8856250f217d4670a520b6759b299424a8a00ee0738eca18bb75", 250692),
                       fact_cache_changes["scripts/release/verify_html_performance_evidence_test.py"][0],
                       fact_cache_changes["scripts/release/verify_html_performance_evidence_test.py"][1]}),
            ("f611152e79c7c499f9b34744d0fd9c8343b58c096cbc2c51a542f3dbbaaf19eb", 262050)),
        "scripts/release/produce_html_performance_evidence.py": (
            frozenset({VALIDATION_COHORT_CHANGES["scripts/release/produce_html_performance_evidence.py"][0],
                       VALIDATION_COHORT_CHANGES["scripts/release/produce_html_performance_evidence.py"][1],
                       ("5b5c62c6ec147f0ed9a541c6eb9d07a883cb3ad90d75eb71bc5b6d8f7750e08c", 254729)}),
            ("a2b967f5b5ce92ec9ed3c6fcc066f68f3e4ae25db1d43de3948a3b9f7786e816", 254802)),
        "scripts/release/produce_html_performance_evidence_test.py": (
            frozenset({VALIDATION_COHORT_CHANGES["scripts/release/produce_html_performance_evidence_test.py"][0],
                       VALIDATION_COHORT_CHANGES["scripts/release/produce_html_performance_evidence_test.py"][1]}),
            ("84393cc63ad448a3c4d561d2c49967cc993ea59ac14d41aeb6cb6bbb603fc074", 297811)),
        "scripts/release/html_performance_host_load_validation.py": (
            frozenset({VALIDATION_COHORT_CHANGES["scripts/release/html_performance_host_load_validation.py"][0],
                       VALIDATION_COHORT_CHANGES["scripts/release/html_performance_host_load_validation.py"][1],
                       ("81ca438ccce28208836dd811438ce81ef27961fe289051110750d1403f2069c5", 10184)}),
            ("289cb02b3c0bbb68db26d76857f97b6cc2d967bc566ad0f80de6a502b9acc700", 10256)),
        "scripts/release/html_performance_host_load_validation_test.py": (
            frozenset({("fee51a18a8da4d4ae389ce9d2a03519b392f60ec03a661567bc3a58cd0c920f1", 14399)}),
            ("bbf598af312742ad33a5585d7b6c800712b48797770858223f2ad274c8ac5239", 14479)),
        "scripts/release/html_performance_source_reuse_test.py": (
            frozenset({("49cc989141f481b7d9e522265d4ebd3c70e7f6ac4f2b98ca36ec187b3a0a3351", 22385),
                       ("56ad9f0eaf412507fe4b6a9325931f3cebe47e385df5d117963407c4c6a88bba", 24025),
                       ("5000e614d0690eaa50a1ffc0c3b1089c9fcf3b8ca13a71ea4bd8c3ea55fd128d", 25819),
                       ("7420dbdd9dc22f325bbaa756ee49ba9a5e08db58761b459b7873463ca47cebf9", 26057),
                       ("8dcddecb1bbb303590ba7ead09c870c1424cfdaf46e93ac2c631dddae1564931", 26691)}),
            ("0a4069d980a508ff27e1b9853db092ca4dd8fe5e3a3b90c9c029bd550130f6d9", 28015)),
    }
    metadata_pair = reviewed_program_changes.get(path)
    if metadata_pair is not None and old in metadata_pair[0] and new == metadata_pair[1]:
        return old_mode == new_mode == "100644"
    budget_targets = {
        "scripts/release/produce_html_performance_evidence.py":
            ("b8e3fdbc94a9f397051a1268060efb1f0ffe1604e2f0adda0f89afa779bdb359", 255681),
        "scripts/release/produce_html_performance_evidence_test.py":
            ("561775468b65b6b0d741a14cb494d8cbb7d6e015c36783ad59e688a628f65c9a", 299581),
        "scripts/release/html_performance_host_load_validation.py":
            ("506a9d3f807144d16e49eb3ab66d938d5795096cf0218df47308f788d449e484", 10328),
        "scripts/release/html_performance_host_load_validation_test.py":
            ("0f21f8e498358bc151949060099ca8680d3d84b14eeb7221b834b345452a464a", 14559),
        "scripts/release/html_performance_source_reuse_test.py":
            ("42fe5276c8fe9f36066218e4859942308141de39dfb0e6428703619787ff40e6", 29428),
    }
    if (metadata_pair is not None and path in budget_targets
            and (old in metadata_pair[0] or old == metadata_pair[1]) and new == budget_targets[path]):
        return old_mode == new_mode == "100644"
    format_changes = {
        "scripts/release/verify_html_performance_evidence_test.py": (
            {("f611152e79c7c499f9b34744d0fd9c8343b58c096cbc2c51a542f3dbbaaf19eb", 262050)},
            ("8da0a7fe5651eaa8cfd22576dbac68c725157931d2a4e06ae619fec948aa1840", 264751)),
        "scripts/release/html_performance_epoch_reuse.py": (
            {("3f50599d284dbe593bb9a1cf02d0fdd633647eb61a3f6be7867b8308d72de3c7", 19278),
             ("958c35a3b8a4e515b42ff45eff5c4019e9d7bb0dc3542e8391ee90151ccb0126", 19876)},
            ("9d537223772252ff9aa6cd43988381cbf79c345a37961c0a842224dbf63c33cd", 20072)),
        "scripts/release/html_performance_epoch_reuse_test.py": (
            {("7af8025db42ec26ab39c0d62eee8423d38d7632f465c431c07553e33bca18a73", 21105),
             ("fffb60fd613de735c238bbc0357711b94aa5c330dc3f10b1ac5e0e15d6f4d938", 23028)},
            ("4acee7faa80a46fa2b150a2f17b354967eac7a3b354f465ddae3d53118947dae", 24339)),
        "scripts/release/html_performance_source_reuse_test.py": (
            {("0a4069d980a508ff27e1b9853db092ca4dd8fe5e3a3b90c9c029bd550130f6d9", 28015),
             ("42fe5276c8fe9f36066218e4859942308141de39dfb0e6428703619787ff40e6", 29428)},
            ("f07cc35c9e42f2c711e8e4a43dbcf1c9770124728d1b76338b6449cd7aefb147", 32633)),
    }
    format_pair = format_changes.get(path)
    if (format_pair is not None and (old in format_pair[0]
            or metadata_pair is not None and old in metadata_pair[0]) and new == format_pair[1]):
        return old_mode == new_mode == "100644"
    promotion_changes = {
        "scripts/release/html_performance_promotion.py": (
            ("8aa8615182f51bcdf53ceecf2301579d4222794c0102298a26c82a92cdc8ed6c", 18397),
            ("0459e7f4f51768f98149e6b47fa04d5a859f663a8d18e584f505c83aa6b0d4ef", 18952)),
        "scripts/release/html_performance_promotion_test.py": (
            ("3407e354b940fabc6288c3c21c034381358e91d14497519fd282e4dbc79b5b37", 13310),
            ("ccf04c8bb03a673e5cd688cfac9f195afc64af1d5b8c078f2228157f128ee0eb", 15430)),
    }
    promotion_pair = promotion_changes.get(path)
    if promotion_pair is not None and (old, new) == promotion_pair:
        return old_mode == new_mode == "100644"
    if (path == "scripts/release/html_performance_source_reuse_test.py"
            and new == ("3f169f0de723a8e28551284ae455a7b95edac4ad18249f989074d4089d76ebd4", 41308)):
        previous = ("6b055a8a3697a99e5ab78653de7c51a4031730a111134b0781cdd3245c5c03ef", 40163)
        return (old_mode == new_mode == "100644" and (old == previous
                or validation_cohort_change(path, old, previous, old_mode, new_mode)))
    if (path == "scripts/release/html_performance_source_reuse_test.py"
            and new == ("6b055a8a3697a99e5ab78653de7c51a4031730a111134b0781cdd3245c5c03ef", 40163)):
        previous = ("994934a37600f9b902eb03a8093a02c370b3abe622aa56ea5b3ec8f48b6a66e3", 34393)
        return (old_mode == new_mode == "100644" and (old == previous
                or validation_cohort_change(path, old, previous, old_mode, new_mode)))
    if (path == "scripts/release/html_performance_source_reuse_test.py"
            and new == ("994934a37600f9b902eb03a8093a02c370b3abe622aa56ea5b3ec8f48b6a66e3", 34393)
            and (format_pair is not None and (old in format_pair[0] or old == format_pair[1])
                 or metadata_pair is not None and (old in metadata_pair[0] or old == metadata_pair[1])
                 or old == budget_targets[path])):
        return old_mode == new_mode == "100644"
    if path in fact_cache_changes and (old, new) == fact_cache_changes[path]:
        return old_mode == new_mode == "100644"
    if path == "scripts/release/html_performance_source_reuse_test.py":
        old_test = ("49cc989141f481b7d9e522265d4ebd3c70e7f6ac4f2b98ca36ec187b3a0a3351", 22385)
        reviewed_tests = {
            ("56ad9f0eaf412507fe4b6a9325931f3cebe47e385df5d117963407c4c6a88bba", 24025),
            ("7420dbdd9dc22f325bbaa756ee49ba9a5e08db58761b459b7873463ca47cebf9", 26057),
            ("8dcddecb1bbb303590ba7ead09c870c1424cfdaf46e93ac2c631dddae1564931", 26691),
        }
        if old == old_test and new in reviewed_tests:
            return old_mode == new_mode == "100644"
        if (old == ("56ad9f0eaf412507fe4b6a9325931f3cebe47e385df5d117963407c4c6a88bba", 24025)
                and new == ("8dcddecb1bbb303590ba7ead09c870c1424cfdaf46e93ac2c631dddae1564931", 26691)):
            return old_mode == new_mode == "100644"
    cohort_predecessors = {
        "scripts/release/verify_html_performance_evidence.py":
            ("630cb08266eb1b24b249bf295b3591cbe87da9529e6f770ce600715ad93ab656", 249838),
        "scripts/release/verify_html_performance_evidence_test.py":
            ("ee5f14f317dc8856250f217d4670a520b6759b299424a8a00ee0738eca18bb75", 250692),
    }
    if path in cohort_predecessors and old == cohort_predecessors[path] and new == fact_cache_changes[path][1]:
        return old_mode == new_mode == "100644"
    test_changes = {
        "scripts/release/verify_html_performance_evidence.py": (
            ("630cb08266eb1b24b249bf295b3591cbe87da9529e6f770ce600715ad93ab656", 249838),
            ("04dc3d5e4d44f239c4bf1e664273d8ad0b4fcd22bb50591bed2535b6d36c9aef", 250526)),
        "scripts/release/html_performance_epoch_reuse.py": (
            ("3f50599d284dbe593bb9a1cf02d0fdd633647eb61a3f6be7867b8308d72de3c7", 19278),
            ("958c35a3b8a4e515b42ff45eff5c4019e9d7bb0dc3542e8391ee90151ccb0126", 19876)),
        "scripts/release/verify_html_performance_evidence_test.py": (
            ("ee5f14f317dc8856250f217d4670a520b6759b299424a8a00ee0738eca18bb75", 250692),
            ("30f00fa2d26a68c4ed95bac4fb7624864fa2588914da65abf606c91fefbfff97", 255806)),
        "scripts/release/html_performance_epoch_reuse_test.py": (
            ("7af8025db42ec26ab39c0d62eee8423d38d7632f465c431c07553e33bca18a73", 21105),
            ("fffb60fd613de735c238bbc0357711b94aa5c330dc3f10b1ac5e0e15d6f4d938", 23028)),
    }
    test_pair = test_changes.get(path)
    if test_pair is not None:
        return old == test_pair[0] and new == test_pair[1] and old_mode == new_mode == "100644"
    return False


def equivalent_program_records(left: Mapping[str, tuple[str, int]],
                               right: Mapping[str, tuple[str, int]]) -> bool:
    """実行しないrelease gateの内容だけを、同一集合の比較から除く。"""
    return left.keys() == right.keys() and all(
        left[path] == right[path] for path in left if path not in RELEASE_ONLY_PROGRAMS
    )


def cache_only_collector_change(old: bytes, current: bytes) -> bool:
    """認証queryの配線だけを移した既知の変更を、byte単位で照合する。"""
    lifecycle = (b'    lifecycle = output.parent / f"{output.name}.lifecycle"\n'
                 b'    gate.require(not lifecycle.exists(), "fresh measurement lifecycle output required")\n'
                 b'    git_query = _make_git_query(lifecycle)\n')
    old_call = b'resume_batch.resolve(), root, fixed_epoch=fixed_epoch,'
    new_call = b'resume_batch.resolve(), root, fixed_epoch=fixed_epoch, git_query=git_query,'
    anchor = b'    if resume_batch is not None:\n'
    marker = b'def produce('
    if old.count(marker) != 1:
        return False
    prefix, body = old.split(marker, 1)
    if body.count(lifecycle) != 1 or body.count(old_call) != 1 or body.count(anchor) != 1:
        return False
    transformed = body.replace(lifecycle, b'', 1).replace(old_call, new_call, 1)
    transformed = transformed.replace(anchor, lifecycle + anchor, 1)
    return prefix + marker + transformed == current


V3_REASSESSMENT_PROGRAM_PAIRS: dict[str, tuple[tuple[str, int], tuple[str, int]]] = {
    'Justfile': (('26a60314d6f99585564ae828130c376726deee1f824c3250423631ba72a5e6b2', 85775), ('dbf845e130ad88db66d467e7ee39665c28b9dae95cc587c98e26035c704f3470', 85775)),
    'scripts/html/performance-history.ts': (('9816902330d44dcdc17acc45f53927083665f8552cd2c31da1b352cd12cbc641', 6855), ('8e85ad5a2cb35702ed3af2db10523bd57ffe4c114d92016efcaec1c7e9a7fb9c', 7578)),
    'scripts/html/performance-history_test.ts': (('7ab7eb760da1c7fc6fe100b0ff7690509ad6377fb3b5a7add7ec43640042f212', 9995), ('14d802c65b4e9963b24726d7ebb307a04f97d73e11cb375c8532df3e8db9dd0b', 11664)),
    'scripts/release/html_performance_host_load_validation.py': (('5002ba1bd630e232a95f799734775aba293fac2410e605fc78b6c618a939f39d', 10472), ('1122db2cae41f9077b03c313d52cb782b0e1063cd289ec611186b629f6acd9be', 10544)),
    'scripts/release/html_performance_memory_evidence.py': (('48f119be884e37ef525da9cb35e6e8e2498514031b63ddeaa9894b67a11828b1', 5188), ('d490af4d926c124c660a2bae335211aea82c69e288d291695170a09fd0affb4a', 5417)),
    'scripts/release/html_performance_memory_evidence_test.py': (('2f3b4d3b4af28f497e46891457cf6ae5f75794aad7078297b1562e6801ff34b3', 5289), ('2d71c16ee3d6a30580fbf4bbe8b23ca72c957275d85e9b15245ec47db93ccd2a', 6782)),
    'scripts/release/html_performance_memory_policy.py': (('c9a513edaa64a0a5983237feef82e1bfaf438e681608eab2b040bb8ba780aef3', 6668), ('c36424f254fc86d00d5f23ab8543154039230253ae6219679a93b50173cb7f0f', 7316)),
    'scripts/release/html_performance_memory_policy_test.py': (('d22479fc07aa834cf6bc3456af9e34fd667b16e9400ae8acc9fe18baa5cfc656', 7128), ('c38d1f9894a42f115f87600be13cae0b5585997682ad5b4af92e86b785c4c8bb', 10900)),
    'scripts/release/html_performance_promotion.py': (('7da57201fd9853c8da3d6445f03fece07577734a2ae5bee89a48a0fe4c19291b', 20638), ('f4f60bef9976e010d402b8c883cafa8bdc0eebcbbf0f33e91ae255e2b68e0805', 22601)),
    'scripts/release/html_performance_promotion_test.py': (('5f5344c9e2b794c8f38d87f837b5bfc9f3e0874d788d3cf0ab9e576b032038e8', 22985), ('6c8381e7c303d4e8f9c75e0bbfdc61299b0f656648b815ed87766402fff67d59', 32877)),
    'scripts/release/html_performance_reference.py': (('6b5dcbf020a1f8b201e29af2294f64033919762aef26b76d7b4ea2abbf42a0bd', 27317), ('f689af203b1807318de7b9ae0954e0e53f98bfb6894f759db7e3ec9980d9eccc', 27512)),
    'scripts/release/html_performance_reference_test.py': (('f94d5702eb640c405918160f9cf29e779fd72dbea93a785c441418716c66d9d3', 25103), ('a997a465a52872b58104323a4672427f17f89b78e9f9c3e93916ff642517d5ea', 27474)),
    'scripts/release/html_performance_source_reuse_cli.py': (('3a0114bf9859067a6d67468c4f48fdf5e1799eb1c6e1d2b63ad9811f74ff692c', 8685), ('e02659cbf4bfac9c8eebb0bb4c45c7254172474e5b5a7f7b9e07654e91bfaa9c', 9866)),
    'scripts/release/html_performance_source_reuse_cli_test.py': (('6e376322520047d3dd16471137e4ec326f1a732fb7d4e724d7eab1c98e7b67d5', 8841), ('e1531684372e64307eb41785dca1ce223898e95a5f65b1be84e31e6ebc916b60', 10590)),
    'scripts/release/produce_html_performance_evidence.py': (('43cad2e67fc7aac594e3d120643fcdcb03e97462f40e04c0fdee8a8c9599c323', 263362), ('2803c2a0a5f212eaeff21334ca2b99e6c80e1fe46f6df232a2a6c20de581d555', 265983)),
    'scripts/release/produce_html_performance_evidence_test.py': (('cfcb9b4ac50cbea8e58c5d261664be0ca60504084b5b2cdc2edf582787a5f1d5', 311180), ('5bd6152ae75b0b40bd0305453c746ef9020d9156b6f1f3c52ff0212f9b7723be', 313113)),
    'scripts/release/verify_html_performance_evidence.py': (('c2d4b4a6b495d5a481006bf8d4f3569036647626c450d8329b9c29f8e129ed72', 258471), ('f9446c16dee3734c80a57dbc1354a9afb99c454b7696582aae81c38aaf9b5cde', 266526)),
    'scripts/release/verify_html_performance_evidence_test.py': (('bd1e61edf9de4b1f783cd5b2bc8f49c1bec8abddba9099df44a08fac600573aa', 285472), ('f863b3302d585e927b8e57c13872bd0b61edf1b54cdb35198cbf2bb3d457876c', 294355)),
}
V3_REASSESSMENT_SELF_PREDECESSORS: frozenset[tuple[str, int]] = frozenset({
    ("a64529247d7c2a2e8d5075bfeed60e2dc12df3bf2abe26aab803b4d6e7d4f331", 60472),
})


def _reassessment_producer_ast_matches(old: bytes, current: bytes, *, v4: bool) -> bool:
    import ast

    changed = {"load_closed_batch", "_reassessment_candidates", "_batch_policy_accepted",
               "_validate_epoch_resume_contexts", "_verify_and_seal_catalog_batch", "produce"}
    if v4:
        changed.add("_finalize_with_failure_receipt")
    def unchanged(source: bytes) -> list[str]:
        tree = ast.parse(source)
        return [ast.dump(node, include_attributes=False) for node in tree.body
                if not (isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name in changed)]
    return unchanged(old) == unchanged(current)


def v3_reassessment_program_change(path: str, old: bytes, current: bytes, *,
                                   old_mode: str | None, new_mode: str | None) -> bool:
    """認証済み取得と新判定の差分だけをV3の明示再評価時に認める。"""
    if old_mode != "100644" or new_mode != "100644":
        return False
    before = (hashlib.sha256(old).hexdigest(), len(old))
    after = (hashlib.sha256(current).hexdigest(), len(current))
    if path == "scripts/release/html_performance_source_reuse.py":
        return (before in V3_REASSESSMENT_SELF_PREDECESSORS
                and after == ("42995d2e0cf3a4d1e988c70699b909cdef73cb2058bddc4e48486746bd6a3dc9", 65857))
    if V3_REASSESSMENT_PROGRAM_PAIRS.get(path) != (before, after):
        return False
    return path != "scripts/release/produce_html_performance_evidence.py" or _reassessment_producer_ast_matches(old, current, v4=False)


# V4のソース確定後、統合時に有限の組を固定する。
V4_REASSESSMENT_PROGRAM_PAIRS: dict[str, frozenset[tuple[tuple[str, int], tuple[str, int]]]] = {
    'Justfile': frozenset({(('26a60314d6f99585564ae828130c376726deee1f824c3250423631ba72a5e6b2', 85775), ('6b5ab582a4acd17d92ac4c3ca997ac637bfce4adf8236d4fb4955337afcc473b', 85802)), (('dbf845e130ad88db66d467e7ee39665c28b9dae95cc587c98e26035c704f3470', 85775), ('6b5ab582a4acd17d92ac4c3ca997ac637bfce4adf8236d4fb4955337afcc473b', 85802))}),
    'scripts/html/performance-history.ts': frozenset({(('8e85ad5a2cb35702ed3af2db10523bd57ffe4c114d92016efcaec1c7e9a7fb9c', 7578), ('d5d49713dad9c362cf264d455d5ddd992f8841638bf243996ae8e96d16f6eac7', 7607)), (('9816902330d44dcdc17acc45f53927083665f8552cd2c31da1b352cd12cbc641', 6855), ('d5d49713dad9c362cf264d455d5ddd992f8841638bf243996ae8e96d16f6eac7', 7607))}),
    'scripts/html/performance-history_test.ts': frozenset({(('14d802c65b4e9963b24726d7ebb307a04f97d73e11cb375c8532df3e8db9dd0b', 11664), ('6f798161f9b210b8c2fcef7dd1c752d40f3e6675c6ef6f7ed7f6dddf01233dd9', 13071)), (('7ab7eb760da1c7fc6fe100b0ff7690509ad6377fb3b5a7add7ec43640042f212', 9995), ('6f798161f9b210b8c2fcef7dd1c752d40f3e6675c6ef6f7ed7f6dddf01233dd9', 13071))}),
    'scripts/release/html_performance_host_load_validation.py': frozenset({(('1122db2cae41f9077b03c313d52cb782b0e1063cd289ec611186b629f6acd9be', 10544), ('527966f32a04e641365dbf06d07002441ffc28ec9527f01cd522a430bd243178', 10616)), (('5002ba1bd630e232a95f799734775aba293fac2410e605fc78b6c618a939f39d', 10472), ('527966f32a04e641365dbf06d07002441ffc28ec9527f01cd522a430bd243178', 10616))}),
    'scripts/release/html_performance_memory_evidence.py': frozenset({(('48f119be884e37ef525da9cb35e6e8e2498514031b63ddeaa9894b67a11828b1', 5188), ('f8a06ac169fea9712fc74923f7d82a0a1425d37c20bfd8c8df05db2aed2d38eb', 5424)), (('d490af4d926c124c660a2bae335211aea82c69e288d291695170a09fd0affb4a', 5417), ('f8a06ac169fea9712fc74923f7d82a0a1425d37c20bfd8c8df05db2aed2d38eb', 5424))}),
    'scripts/release/html_performance_memory_evidence_test.py': frozenset({(('2d71c16ee3d6a30580fbf4bbe8b23ca72c957275d85e9b15245ec47db93ccd2a', 6782), ('316cf7ceaba4cab53336f9ef14fc0c84bbf38688521815f53b607b804e714324', 7139)), (('2f3b4d3b4af28f497e46891457cf6ae5f75794aad7078297b1562e6801ff34b3', 5289), ('316cf7ceaba4cab53336f9ef14fc0c84bbf38688521815f53b607b804e714324', 7139))}),
    'scripts/release/html_performance_memory_policy.py': frozenset({(('c36424f254fc86d00d5f23ab8543154039230253ae6219679a93b50173cb7f0f', 7316), ('8864ab51bea24bcfd38adc52c24550121d0981b7b8cd189ba032eeddbf5b9ca2', 7506)), (('c9a513edaa64a0a5983237feef82e1bfaf438e681608eab2b040bb8ba780aef3', 6668), ('8864ab51bea24bcfd38adc52c24550121d0981b7b8cd189ba032eeddbf5b9ca2', 7506))}),
    'scripts/release/html_performance_memory_policy_test.py': frozenset({(('c38d1f9894a42f115f87600be13cae0b5585997682ad5b4af92e86b785c4c8bb', 10900), ('8aa7a51fef9932a6112c04a1d14eb7da84596deb01434b60fe92444222956be2', 14655)), (('d22479fc07aa834cf6bc3456af9e34fd667b16e9400ae8acc9fe18baa5cfc656', 7128), ('8aa7a51fef9932a6112c04a1d14eb7da84596deb01434b60fe92444222956be2', 14655))}),
    'scripts/release/html_performance_promotion.py': frozenset({(('7da57201fd9853c8da3d6445f03fece07577734a2ae5bee89a48a0fe4c19291b', 20638), ('fd462f5318069d00173250aa09aa0ebd65a11d3efe0a049b1f4ac8848ba2937b', 22979)), (('f4f60bef9976e010d402b8c883cafa8bdc0eebcbbf0f33e91ae255e2b68e0805', 22601), ('fd462f5318069d00173250aa09aa0ebd65a11d3efe0a049b1f4ac8848ba2937b', 22979))}),
    'scripts/release/html_performance_promotion_test.py': frozenset({(('5f5344c9e2b794c8f38d87f837b5bfc9f3e0874d788d3cf0ab9e576b032038e8', 22985), ('f74bd1a78bd9b85d8db09029c9b6f4f9aa846adf752665787de0d85ea5f40802', 35275)), (('6c8381e7c303d4e8f9c75e0bbfdc61299b0f656648b815ed87766402fff67d59', 32877), ('f74bd1a78bd9b85d8db09029c9b6f4f9aa846adf752665787de0d85ea5f40802', 35275))}),
    'scripts/release/html_performance_reference.py': frozenset({(('6b5dcbf020a1f8b201e29af2294f64033919762aef26b76d7b4ea2abbf42a0bd', 27317), ('821479c23a10c5efa1cc2a86e36516fd00b53a63dd1824825add66c493adbd4e', 27578)), (('f689af203b1807318de7b9ae0954e0e53f98bfb6894f759db7e3ec9980d9eccc', 27512), ('821479c23a10c5efa1cc2a86e36516fd00b53a63dd1824825add66c493adbd4e', 27578))}),
    'scripts/release/html_performance_reference_test.py': frozenset({(('a997a465a52872b58104323a4672427f17f89b78e9f9c3e93916ff642517d5ea', 27474), ('2a2d9eb8956c8716ffe598d251b7f86dc748c28c4a6201035b617003f459cba3', 29049)), (('f94d5702eb640c405918160f9cf29e779fd72dbea93a785c441418716c66d9d3', 25103), ('2a2d9eb8956c8716ffe598d251b7f86dc748c28c4a6201035b617003f459cba3', 29049))}),
    'scripts/release/html_performance_source_reuse_cli.py': frozenset({(('3a0114bf9859067a6d67468c4f48fdf5e1799eb1c6e1d2b63ad9811f74ff692c', 8685), ('d49416a4c83e18f177a0572b510a925ef41276e84d2d3ba8e735c732f06028a2', 10235)), (('e02659cbf4bfac9c8eebb0bb4c45c7254172474e5b5a7f7b9e07654e91bfaa9c', 9866), ('d49416a4c83e18f177a0572b510a925ef41276e84d2d3ba8e735c732f06028a2', 10235))}),
    'scripts/release/html_performance_source_reuse_cli_test.py': frozenset({(('6e376322520047d3dd16471137e4ec326f1a732fb7d4e724d7eab1c98e7b67d5', 8841), ('5b5a97a585e37f4583f6321935847acfc092d84dd8f1f8d194c3df30f4408554', 11721)), (('e1531684372e64307eb41785dca1ce223898e95a5f65b1be84e31e6ebc916b60', 10590), ('5b5a97a585e37f4583f6321935847acfc092d84dd8f1f8d194c3df30f4408554', 11721))}),
    'scripts/release/local_html_performance_evidence.py': frozenset({(('6f47f76911050667f0480902ce121b04e1befec6d1dcef39b9eb92b7dad8d29d', 17232), ('34a2e56ecac228ca5bd9ecfd2c605a5c182773402652ceadc9019d031ebfd3ca', 17378))}),
    'scripts/release/local_html_performance_evidence_test.py': frozenset({(('97a51614a478f19e38a923813d75a913dc7ec6c659338c904109d3261c3f68df', 14399), ('d019f3fb492a3cae377a7c98a0046e087fd86359c3ee661a377fd061a07c99a7', 18001))}),
    'scripts/release/produce_html_performance_evidence.py': frozenset({(('2803c2a0a5f212eaeff21334ca2b99e6c80e1fe46f6df232a2a6c20de581d555', 265983), ('43d967863089f9c2267326f2dde177db898861d1c0634f2e951eda7b212cbbff', 269141)), (('43cad2e67fc7aac594e3d120643fcdcb03e97462f40e04c0fdee8a8c9599c323', 263362), ('43d967863089f9c2267326f2dde177db898861d1c0634f2e951eda7b212cbbff', 269141))}),
    'scripts/release/produce_html_performance_evidence_test.py': frozenset({(('5bd6152ae75b0b40bd0305453c746ef9020d9156b6f1f3c52ff0212f9b7723be', 313113), ('09e332a5143e9a3ec1edadc5d7b7d8d4ef55973de8d6c0ed3619643d860d9ac8', 319469)), (('cfcb9b4ac50cbea8e58c5d261664be0ca60504084b5b2cdc2edf582787a5f1d5', 311180), ('09e332a5143e9a3ec1edadc5d7b7d8d4ef55973de8d6c0ed3619643d860d9ac8', 319469))}),
    'scripts/release/verify_html_performance_evidence.py': frozenset({(('c2d4b4a6b495d5a481006bf8d4f3569036647626c450d8329b9c29f8e129ed72', 258471), ('73ffb34fc4fd5d46a698059879e29753f546c10f53815b20df5d7a9d17468060', 268550)), (('f9446c16dee3734c80a57dbc1354a9afb99c454b7696582aae81c38aaf9b5cde', 266526), ('73ffb34fc4fd5d46a698059879e29753f546c10f53815b20df5d7a9d17468060', 268550))}),
    'scripts/release/verify_html_performance_evidence_test.py': frozenset({(('bd1e61edf9de4b1f783cd5b2bc8f49c1bec8abddba9099df44a08fac600573aa', 285472), ('868e20d17e195e38d8e17cd65acca868c54d7fae3d2a92e3ee7f19d2253cfd5e', 301671)), (('f863b3302d585e927b8e57c13872bd0b61edf1b54cdb35198cbf2bb3d457876c', 294355), ('868e20d17e195e38d8e17cd65acca868c54d7fae3d2a92e3ee7f19d2253cfd5e', 301671))}),
}


def v4_reassessment_program_change(path: str, old: bytes, current: bytes, *,
                                   old_mode: str | None, new_mode: str | None) -> bool:
    if old_mode != "100644" or new_mode != "100644":
        return False
    before = (hashlib.sha256(old).hexdigest(), len(old))
    after = (hashlib.sha256(current).hexdigest(), len(current))
    if path == "scripts/release/html_performance_source_reuse.py":
        return (before in V3_REASSESSMENT_SELF_PREDECESSORS
                or before == ("42995d2e0cf3a4d1e988c70699b909cdef73cb2058bddc4e48486746bd6a3dc9", 65857)) and (current == Path(__file__).read_bytes() or before != after and after == ("85d9ff1f649fcef4b85cdd7634b4ce4c711a4e567d7203eecbea33793954b4eb", 74484))
    if (before, after) not in V4_REASSESSMENT_PROGRAM_PAIRS.get(path, frozenset()):
        return False
    return path != "scripts/release/produce_html_performance_evidence.py" or _reassessment_producer_ast_matches(old, current, v4=True)


def known_auth_program_change(path: str, old: bytes, current: bytes) -> bool:
    """この移行でレビューする認証・輸送コードの実体だけを許可する。"""
    import ast
    import inspect

    if path == "scripts/release/html_performance_source_reuse.py":
        old_sha = hashlib.sha256(old).hexdigest()
        current_sha = hashlib.sha256(current).hexdigest()
        history = ("8c3503e588e2e7ab93770a86c4e1ffb0c10ea463e2d98c808d2e6fef1a6895bd",
                   "d3d6a94ad5cb0465a46cf0b98fb74a4fe9cb46c5a9ac5a72ca9fa4824e762015",
                   "e11014eb370b864d812928235c7b033fca37acb8f2e0d8341bfd86c55c2672b2",
                   "37c6a3b87c0d5367d2144b8850f9cd7b97294e92fe32886fbb408768033d3ad2",
                   "0bde8075042a47d5601044e860d997f49e8aaf85a8b67a1c6ea862e37df90844",
                   "10225a74072a6d30b58ca01d3a6e93f0797d75bc1b700b963c725bc4192a8384",
                   "4a57d835945ea1b095b039fa565f80fbc01ee7e5d231bad1c7c560c53d94d6b0",
                   "d92d9865cd7e501fccf45202d3eaee6df795f2f0d27bf0cc9d6d284ca5aa4099")
        if old_sha in history:
            return (current == Path(__file__).read_bytes()
                    or current_sha == "85d9ff1f649fcef4b85cdd7634b4ce4c711a4e567d7203eecbea33793954b4eb"
                    or current_sha in history[history.index(old_sha) + 1:])
        if old_sha != "847b5541ecc81d533eaf510e1675b7a2b667853a7dc8793f34387470d9dce426":
            return False
        if current_sha in {
            "3b98d203dd23f65513ab4d4cb60af4b63ca49ff2468705c34982b588877c9e97",
            "3fee6e9a540c90a10ad8f02cd95e6edddaf4b929c69f66cb2a4643a7b96594ae",
            "ddc12b9fc326b4fea119ccbcc807dbff65031bb9a35ae1cf44487ecabbd23796",
            "9a39e0d8b2fa2f3bf78504f29cdfa68d1db71028c109de9712fd698486a8bd1f",
            "dbf7f198d1dbc28ca3a8067af0fde153640f4208d4c9bc18c73896e137668055",
            "d3d6a94ad5cb0465a46cf0b98fb74a4fe9cb46c5a9ac5a72ca9fa4824e762015",
        }:
            return True
        if current != Path(__file__).read_bytes():
            return False
        old_tree = ast.parse(old)
        current_tree = ast.parse(current)
        additions = {"cache_only_collector_change", "known_auth_program_change", "_specific_source_changes",
                     "validation_cohort_change"}
        definitions = [node for node in current_tree.body
                       if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name in additions]
        if len(definitions) != len(additions) or {node.name for node in definitions} != additions:
            return False
        trusted: dict[str, Callable[..., object]] = {
            "cache_only_collector_change": cache_only_collector_change,
            "known_auth_program_change": known_auth_program_change,
            "_specific_source_changes": _specific_source_changes,
            "validation_cohort_change": validation_cohort_change,
        }
        for node in definitions:
            actual = ast.dump(node, include_attributes=False)
            expected_nodes = ast.parse(inspect.getsource(trusted[node.name])).body
            if len(expected_nodes) != 1 or actual != ast.dump(expected_nodes[0], include_attributes=False):
                return False
        old_validators = [node for node in old_tree.body
                          if isinstance(node, ast.FunctionDef) and node.name == "validate_reuse_source"]
        current_validators = [node for node in current_tree.body
                              if isinstance(node, ast.FunctionDef) and node.name == "validate_reuse_source"]
        if len(old_validators) != 1 or len(current_validators) != 1:
            return False
        current_validator = current_validators[0]
        fragment = ast.get_source_segment(current.decode("utf8"), current_validator)
        if fragment is None:
            return False
        replacements = (
            ("    approved_changes, new_test_files = _specific_source_changes(\n        root, measured_head, current, measured, old_modes, new_modes, git_bytes,\n        allow_test_changes=allow_test_changes,\n    )\n    old_checked = {path: mode for path, mode in old_modes.items()\n", "    old_checked = {path: mode for path, mode in old_modes.items()\n"),
            ("    new_checked = {path: mode for path, mode in new_modes.items()\n                   if (program_records is None or not is_program(path)) and path not in new_test_files}\n", "    new_checked = {path: mode for path, mode in new_modes.items()\n                   if program_records is None or not is_program(path)}\n"),
            ("    current_checked = {path for path in current\n                       if (program_records is None or not is_program(path)) and path not in new_test_files}\n", "    current_checked = {path for path in current if program_records is None or not is_program(path)}\n"),
            ("    test_changes = set(changed) - ALLOWED - program_changes - approved_changes\n", "    test_changes = set(changed) - ALLOWED - program_changes\n"),
            ("    if any(old_modes.get(path) != \"100644\" for path in changed\n           if path not in program_changes and path not in new_test_files):\n", "    if any(old_modes.get(path) != \"100644\" for path in changed if path not in program_changes):\n"),
            ("                              bool(test_changes or new_test_files), bool(program_changes))", "                              bool(test_changes), bool(program_changes))"),
        )
        for added, original in replacements:
            if fragment.count(added) != 1:
                return False
            fragment = fragment.replace(added, original, 1)
        normalized = ast.parse(fragment).body
        if len(normalized) != 1 or ast.dump(normalized[0], include_attributes=False) != ast.dump(
                old_validators[0], include_attributes=False):
            return False
        current_tree.body = [old_validators[0] if node is current_validator else node
                             for node in current_tree.body]
        current_tree.body = [node for node in current_tree.body
                             if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
                             or node.name not in additions]
        constants = {"VALIDATION_COHORT_ADDITIONS": VALIDATION_COHORT_ADDITIONS,
                     "VALIDATION_COHORT_CHANGES": VALIDATION_COHORT_CHANGES}
        for name, value in constants.items():
            nodes = [node for node in current_tree.body if isinstance(node, ast.AnnAssign)
                     and isinstance(node.target, ast.Name) and node.target.id == name]
            if len(nodes) != 1 or nodes[0].value is None or ast.literal_eval(nodes[0].value) != value:
                return False
            current_tree.body.remove(nodes[0])
        return ast.dump(old_tree, include_attributes=False) == ast.dump(current_tree, include_attributes=False)
    pins = {
        "scripts/release/local_macos_evidence.py": (
            "de2b7db8aa2e1b2469a79819aed9063f6c46fdfffd8466ed7a744984a63b9d9d",
            "78ae5f111e5d7774fee2cb7e5fba6a255b74720265e6fe0de5480bb71168c1a3"),
        "scripts/quality/owned-validation-contract.ts": (
            "68a287400d2f7c8427d44c7be565d9b8ed38153e1c76d117ad29b6cdb167ba95",
            frozenset({"c3ec88e882c759907f8f2ce69e9556a4fcdd94cdba6c45044e07fd745b13d33d",
                       "bea812c143b520f502c1d02840ad3d8d0233ae4016856adf5268df5cdd21e34b"})),
        "scripts/quality/owned-validation-contract_test.ts": (
            "38f60e211b50840f47cb4372123a526e99c76e062f7cc0022155d96fc74079e2",
            frozenset({"4b5c0fe741e31c8b10d366804f957e63211e0b0e7ecfff782aee57d2fd424a4e",
                       "d3a1199e399a20562d90b0e5e614e1b4491319b5073a91e4970570fe2deaedc3"})),
        "scripts/release/html_performance_reuse_entry_test.py": (
            "6b0518f9992ac91d7fc541cbbfc2563bbfc19095f1d00dcf4600dd9361c4d036",
            "ef435de25b26fefabcb2e3e3b627633b539dd6fa473e7f493a6e4bef13348e73"),
        "scripts/release/html_performance_source_reuse_test.py": (
            "bc44642c01d96526490a2ba97bce9b66bd354c942851d2ce772071b3416f8304",
            frozenset({"81a7db4b3307e566f0e0eb59448248aec60d49430ab76e6d9b123eed9727996e",
                       "49cc989141f481b7d9e522265d4ebd3c70e7f6ac4f2b98ca36ec187b3a0a3351",
                       "5000e614d0690eaa50a1ffc0c3b1089c9fcf3b8ca13a71ea4bd8c3ea55fd128d",
                       "56ad9f0eaf412507fe4b6a9325931f3cebe47e385df5d117963407c4c6a88bba",
                       "7420dbdd9dc22f325bbaa756ee49ba9a5e08db58761b459b7873463ca47cebf9",
                       "8dcddecb1bbb303590ba7ead09c870c1424cfdaf46e93ac2c631dddae1564931",
                       "0a4069d980a508ff27e1b9853db092ca4dd8fe5e3a3b90c9c029bd550130f6d9",
                       "42fe5276c8fe9f36066218e4859942308141de39dfb0e6428703619787ff40e6",
                       "f07cc35c9e42f2c711e8e4a43dbcf1c9770124728d1b76338b6449cd7aefb147",
                       "994934a37600f9b902eb03a8093a02c370b3abe622aa56ea5b3ec8f48b6a66e3",
                       "6b055a8a3697a99e5ab78653de7c51a4031730a111134b0781cdd3245c5c03ef"})),
        "scripts/release/html_performance_epoch_reuse.py": (
            "33a05103fc5100c7eb2e9d10368785c8ab20e6f8308ddc53f0ae95f63c573f1c",
            frozenset({"3f50599d284dbe593bb9a1cf02d0fdd633647eb61a3f6be7867b8308d72de3c7",
                       "958c35a3b8a4e515b42ff45eff5c4019e9d7bb0dc3542e8391ee90151ccb0126",
                       "9d537223772252ff9aa6cd43988381cbf79c345a37961c0a842224dbf63c33cd"})),
        "scripts/release/html_performance_epoch_reuse_test.py": (
            "7af8025db42ec26ab39c0d62eee8423d38d7632f465c431c07553e33bca18a73",
            frozenset({"fffb60fd613de735c238bbc0357711b94aa5c330dc3f10b1ac5e0e15d6f4d938",
                       "4acee7faa80a46fa2b150a2f17b354967eac7a3b354f465ddae3d53118947dae"})),
        "scripts/release/html_performance_host_load_validation.py": (
            "eaf150043f1361b9a5d75ee5eb4f6348c6541564c1299a091265f446cd445f74",
            frozenset({"546885df19bbaf8d40b6428af6dcbece6c4ba59c7f73f5bb4aa7ec72a0339269",
                       "80b59b6d1fb10ba7953e2f598bed2928483a8ee50aea09ae1992796f6239b87a",
                       "50c1fd0439f46472bb789865ecfd83714f6feaaaa8c882521880dab6710b3659",
                       "81ca438ccce28208836dd811438ce81ef27961fe289051110750d1403f2069c5",
                       "289cb02b3c0bbb68db26d76857f97b6cc2d967bc566ad0f80de6a502b9acc700",
                       "506a9d3f807144d16e49eb3ab66d938d5795096cf0218df47308f788d449e484",
                       "5002ba1bd630e232a95f799734775aba293fac2410e605fc78b6c618a939f39d"})),
        "scripts/release/html_performance_host_load_validation_test.py": (
            "977394a86e45b307f9939a952aa6ef66d3cf770154564794d7196036aa6c2497",
            frozenset({"a550c3e89979dda00597eb3497bf627c793a8a5228c88bc740976526ad70aa66",
                       "fee51a18a8da4d4ae389ce9d2a03519b392f60ec03a661567bc3a58cd0c920f1",
                       "bbf598af312742ad33a5585d7b6c800712b48797770858223f2ad274c8ac5239",
                       "0f21f8e498358bc151949060099ca8680d3d84b14eeb7221b834b345452a464a"})),
        "scripts/release/produce_html_performance_evidence.py": (
            "d4d85ae7e2120e3720ef31afe6a7228011385d0267ec770060e68f606e21388e",
            frozenset({"490233a4797d9690f7393000cfeda1ceb4d806e1a7df2c588bbb300d8cc1dae5",
                       "bffd5b7781ea1650185d2756a90c4c7350d7708726d7482cf6fff397c4b14aa8",
                       "80ded11e1b2984635cd455aa55dd926eefbbd03759d8c9eccd719dc16bfd7d45",
                       "5b5c62c6ec147f0ed9a541c6eb9d07a883cb3ad90d75eb71bc5b6d8f7750e08c",
                       "a2b967f5b5ce92ec9ed3c6fcc066f68f3e4ae25db1d43de3948a3b9f7786e816",
                       "b8e3fdbc94a9f397051a1268060efb1f0ffe1604e2f0adda0f89afa779bdb359"})),
        "scripts/release/produce_html_performance_evidence_test.py": (
            "1ed2ab04cc2fccc6674b0eb565e097d9df37d44ca66d5b6fc70e12d7583e2fe2",
            frozenset({"52f4b3cb14c379b5fb3c260d170b10024d30377bf63ad33b4084a8fc3137db9b",
                       "f519cb6810e911a59c5eb0ca3b41ba410aa0d98ed9cc23e600822b4d38ab51ed",
                       "7ff9328bebff71189ca1c32719923d50688665b88e284f0a2c4d16ff4c216cad",
                       "ea9484f7714c5279d7f8880fb4a499e4fd24fb8b7fa7818b227dd5075cdf9cc9",
                       "84393cc63ad448a3c4d561d2c49967cc993ea59ac14d41aeb6cb6bbb603fc074",
                       "561775468b65b6b0d741a14cb494d8cbb7d6e015c36783ad59e688a628f65c9a"})),
        "scripts/release/verify_html_performance_evidence.py": (
            "d9d93a641fd5e5dc4db5a0135cffa2073c84f34da132cedfb9821dace49a59aa",
            frozenset({"71fa1bc5e34a3d9432d2923d14e0967f7ae2d6e5f879941eb3f858668b1da86e",
                       "5eb3892fb7d508586f43fe62df8ecfd603e3fb540af442a8abb6ceccc7e022c2",
                       "30a772d8b1488a764185ab0761afb555cd65d798a2696674dde0821465ce4f83",
                       "ee23eb6dbd56ca3dcd6ae00b4f79a69675b6ec9cd408b7158d16f1ded88d4d60",
                       "630cb08266eb1b24b249bf295b3591cbe87da9529e6f770ce600715ad93ab656",
                       "04dc3d5e4d44f239c4bf1e664273d8ad0b4fcd22bb50591bed2535b6d36c9aef",
                       "65beb54c881365eafb607d9ced7f345f6fdc63221386bd16dc403e3a8b835fd2",
                       "610b1005cca73b8b3a8242e6b3219db65d0526225ae7cc5fb56032bcd8b0f151"})),
        "scripts/release/verify_html_performance_evidence_test.py": (
            "9f423f9edbd3fa5ef2a0f1fd1d28a2d2a61f968498650f65dd3f0e5f323be4b3",
            frozenset({"953044024351425627b8798fcbbc3d5f9216ac49d78e7c63075cd034536584e0",
                       "2daf590b500a480c83615bdb436a57c77834546a183c77638d89f2d8a74a20c7",
                       "91c7380a5516b550acfeffbbc165bb8e5b1c5480c14e4beff11609fd4898ebe5",
                       "ee5f14f317dc8856250f217d4670a520b6759b299424a8a00ee0738eca18bb75",
                       "095ccaa79b72cb540d37e8ec24e77937a67b539be074a3e019edee27aaac22f3",
                       "30f00fa2d26a68c4ed95bac4fb7624864fa2588914da65abf606c91fefbfff97",
                       "aa3211d3807addbacad8290e5724f46302f9a72451a96732b3cf070fe241c899",
                       "f611152e79c7c499f9b34744d0fd9c8343b58c096cbc2c51a542f3dbbaaf19eb",
                       "8da0a7fe5651eaa8cfd22576dbac68c725157931d2a4e06ae619fec948aa1840"})),
    }
    expected = pins.get(path)
    if expected is None or expected[0] != hashlib.sha256(old).hexdigest():
        return False
    allowed = expected[1]
    current_sha = hashlib.sha256(current).hexdigest()
    return current_sha in allowed if isinstance(allowed, frozenset) else current_sha == allowed


# この定数だけをゼロへ正規化してモジュール全体を固定する。
SELF_MODULE_CANONICAL_SHA256 = "3d7a428b40c8480fc5d458371cb84da4485f807bf35d99f63feeff5baf3cd830"
SAME_BINARY_REVIEWED_BASE = "81570a15263b2d47206c2d953e8721f55e255eb3"
SAME_BINARY_REVIEWED_ADDITIONS: dict[str, tuple[str, int]] = {
    "scripts/build/cargo_capacity_guard_root_scope_test.py":
        ("4b2c62c1f6921fe9ddb82e5c023ed6e6720c51a4c1f96ce95b41158656858955", 3743),
}

# 実行中 checker は品質ゲートとレビューで信頼し、入力 bytes の改変を拒否する。
_SAME_BINARY_REVIEWED_FILES = {
    "scripts/release/publish-crates.sh": ("27a3543b8af5114c1958612d68a4d6ebc60092ea83ccec761aa9ab96b615a654", "fb69ddf60e8a667340b2147f1a2c5b4bdf652766f398ec7c0b8e1ebb8772ebd6"),
    "scripts/release/publish_crates_test.py": ("924d5e48839b98401435860ed64d3e91f9158abe0b9136f9c91d3b63b0343686", "70f5a79be0545f6868ae290dade37ce8d4ee961d29480f4fe3d536e5e375c629"),
    "scripts/hooks/run_locked_command.py": ("a13ecdd3a5795349796a738f144a64cf7e2599f33a93d1dc994ba86724227336", "a3d2091307219fdc6069881c5919d0e7167630b709c17f272a1af2aa372e2f25"),
    "scripts/hooks/run_locked_command_test.py": ("1b1c81bfcb0887c93db0c24b265230652b19a85e87a77b968c72787d1bf2d2f5", "d97ea778a522a13137997c1d6f43a1e9be2945d4f02cf9c4f6a0d37d6870f31e"),
    "scripts/build/cargo_capacity_guard.py": ("f9f8e97cf0628e5281c45be3dbc67c24f08dbb2ae281909438619a50abff6448", "d108bba44b0575cfa5b1122860fbd1e6e31613905fc1f3d406e551a1a57e7ff8"),
    "scripts/build/cargo_capacity_guard_test.py": ("f01e32aca9cd651a9dea9cf5cdddc6c5b97e255faf9b55c7e1a387766e2317ce", "a840886e57e3f7ca600c4cfbfecbce299be73a59751313c0b1813498cda90ed5"),
    "scripts/build/cargo_capacity_guard_state_integration_test.py": ("c833fa2261b950dbc6045f7a9513a394959e34811053901dceeb3be015747261", "91cb48ba570f7249506d6ed959d7a2f6ce2f7b3930df2b179f5b78653a0d787c"),
    "scripts/release/native_coverage_result.py": ("b9ca6c28772c561d2c961c7b4b6b869e1f5dc4990781d6cfb8f3e01f35e12f6d", "3000a0aea2245ee95c298c41b95594034bcfb8fa1c583db4041324a96d576084"),
    "scripts/release/native_coverage_result_test.py": ("9a2b81372f5713f222d45bdd81526dd2562e7300fb5b1b33a33f93d9dd938c0b", "3f172d60dc835c7564c23728cab45c79dc070fdb79a99e3585fa8a2851a8e69f"),
    "scripts/release/native_nextest_coverage_commands.py": ("9f2b9b5ca377795692ecf57473532a821f1f6aaa9d417b1d9e0763a7f61c961f", "5f5aa41d7ab72b9dcd5c921cf9cfe1d2ea2ca1d27a23e797cf2b767dc99d3b47"),
    "scripts/release/native_nextest_root_coverage_test.py": ("fcaef83f88da28667b329c0478b1df2a19aa535c4d85ff91536894a0a1e3e936", "6fd8053fcf727b3082398caceee6643216216842a01dd5c4027f689114a3e5af"),
    "scripts/release/html_performance_remote_evidence.py": ("e6b82ad9987bc409f1ad6dbc9b3aa9d790f665b111538152dfecd2628efbf712", "f23053b1cd1fa3d1c5a6729659edab0e24b374cb2893df9f1a7aaeacdcd92c6f"),
    "scripts/release/html_performance_host_load_validation.py": ("527966f32a04e641365dbf06d07002441ffc28ec9527f01cd522a430bd243178", "a239f5caa5f0a55a7c182532620e43a93364cc2087c3b75880c4778329acf12b"),
    "scripts/release/html_performance_host_load_validation_test.py": ("68e27e9b1b715bbed15eb260ac45b0449bdd4d24e76897964e06eafd8a98acc0", "cd47e6b4bc807c64db170a16c7add7514875857a5f1bc3914419271764f44742"),
    "scripts/release/produce_html_performance_evidence.py": ("43d967863089f9c2267326f2dde177db898861d1c0634f2e951eda7b212cbbff", "213afcfcf895ee1f048564e70d10668187e0829eee49cbb3fbdc384648777e1b"),
    "scripts/release/produce_html_performance_evidence_test.py": ("09e332a5143e9a3ec1edadc5d7b7d8d4ef55973de8d6c0ed3619643d860d9ac8", "e4c92f155972b92f4b9a46ca4fd735f1ff89d7e6bca75357460b6298dd33da03"),
    "scripts/release/verify_html_performance_evidence.py": ("73ffb34fc4fd5d46a698059879e29753f546c10f53815b20df5d7a9d17468060", "c3b06e39f36228753283e713ac39698feb578ba5425f5e93b4d66a7fad417746"),
    "scripts/release/verify_html_performance_evidence_test.py": ("7c0f9385c5b896a3ceb4f119a97f2329f9083cd9853028fc8663dade2339bf86", "887bcbc646e293938df96a9b38b2a91d29f92a6c463eaef9f69e0e374a83f56b"),
    "scripts/release/html_performance_source_reuse_test.py": ("3f169f0de723a8e28551284ae455a7b95edac4ad18249f989074d4089d76ebd4", "91270d5087c058eb9e5d341ba1462c634954037fe2c50c3716cc0fb7a7fe6b45"),
    "scripts/release/html_performance_source_reuse_cli.py": ("d49416a4c83e18f177a0572b510a925ef41276e84d2d3ba8e735c732f06028a2", "73e202554a1893741b774fced28d59a598432715f59fa5c935001ed70db2dcf0"),
    "scripts/release/html_performance_source_reuse_cli_test.py": ("5b5a97a585e37f4583f6321935847acfc092d84dd8f1f8d194c3df30f4408554", "3a7b680aebd158c594d66c8d7224d51f76bcf0f0922ead6142154a7160cf74b3"),
    "scripts/release/html_performance_reuse_entry_test.py": ("ef435de25b26fefabcb2e3e3b627633b539dd6fa473e7f493a6e4bef13348e73", "06be7d060ee3ff06ffa187b4575fbdbdbd42d0d16de8877f4b0452eceabc5695"),
    "scripts/release/local_html_performance_evidence_test.py": ("d019f3fb492a3cae377a7c98a0046e087fd86359c3ee661a377fd061a07c99a7", "fab4f5a15ba5d3509c25db5789c8af2c98a3a01af4a276ea79308febf38e71aa"),
    "scripts/runtime-assets/svg-engine-provenance.ts": ("1c08a971ff2e98f5a99c805846bfefc3a11acc1dd6db900a216bff1a37a8d4ea", "f57f6244c0b4557592b0a2b2eaca20ca0906d27deff4c20796e6025b9def57a5"),
}


def reviewed_same_binary_self(current: bytes, *, allow_historical: bool = False) -> bool:
    pattern = rb'(?m)^SELF_MODULE_CANONICAL_SHA256 = "([0-9a-f]{64})"$'
    matches = list(re.finditer(pattern, current))
    if len(matches) != 1:
        return False
    match = matches[0]
    declared = match.group(1).decode("ascii")
    historical = {
        "d8bb47141e9ffa6fc1c405dadd6011823cadcbe8355903223251faf50442e7e2":
            "ded22671818bda6a5321d1abb38d129ed8cf135c0320b28956cee2f969cefb81",
        "90adfdc15b010fd330301c94b464f46dbfa5e823e1805118d66736cc898a545b":
            "e74539aa42a84ded142f4659dd8cef84f731a5da3f6e92b178987417f49dc960",
        "cc8ea5b99cd1afb2d9a7270775a97ff7ab326a9daee3562906e06ac724c9199f":
            "4a07266dbe727c9a6fef6ba07d09850422ded4b517f22425a70a0d8a627dfac8",
        "68679b525416b96b51a4f4072154d70881c3596ed86ac52eb07121d577dd394a":
            "91b449135672db61560b1b749a9aad8a98511f5d8afcdcf40a59043d620e3fc7",
        "c2fb007d478dc3acc1996b9d91b189363a39c8d6f2e4be969f26046935dfec70":
            "84c6f083827bb1aeb1afc52fe41ed38444bf52bcce20da1394f63e3407fdd22a",
    }
    if declared != SELF_MODULE_CANONICAL_SHA256 and (
        not allow_historical or historical.get(hashlib.sha256(current).hexdigest()) != declared
    ):
        return False
    normalized = current[:match.start(1)] + b"0" * 64 + current[match.end(1):]
    return hashlib.sha256(normalized).hexdigest() == declared


def reviewed_same_binary_migration(path: str, old: bytes, current: bytes, *,
                                   old_mode: str | None, new_mode: str | None) -> bool:
    if old_mode != "100644" or new_mode != "100644":
        return False
    if path == "scripts/release/html_performance_source_reuse.py":
        return (hashlib.sha256(old).hexdigest()
                == "85d9ff1f649fcef4b85cdd7634b4ce4c711a4e567d7203eecbea33793954b4eb"
                and reviewed_same_binary_self(current, allow_historical=True))
    historical_pairs = {
        "scripts/release/html_performance_source_reuse_test.py": (
            "3f169f0de723a8e28551284ae455a7b95edac4ad18249f989074d4089d76ebd4",
            frozenset({"d5ed585d010c7ae5110a77245a2213c3387f964fafa388d5baebff43c223428d",
                       "7d3a16959b8681a12cdcaf759d12916d5b5cc8b9ba137f051da76a338e191555",
                       "ae8d52c5b22cf934d710b8c099a0ffbe260c12c845bca34e2bd9c62adfde1d0c",
                       "7f385f64a95de6ed5c3ee402fb0ea1d08e980b2df44babd9e3423e92b6d4a763"})),
        "scripts/release/verify_html_performance_evidence_test.py": (
            "7c0f9385c5b896a3ceb4f119a97f2329f9083cd9853028fc8663dade2339bf86",
            frozenset({"3f562c30a3995d06cd5c54c7e5c1d1b630d8c45b7515a5a364fc11b9de4e07f6"})),
        "scripts/release/html_performance_source_reuse_cli.py": (
            "d49416a4c83e18f177a0572b510a925ef41276e84d2d3ba8e735c732f06028a2",
            frozenset({"cf246f950aec2619cf27d4605a411d268d52e5f84f34d7815ab5c944a17b00b9"})),
        "scripts/release/html_performance_source_reuse_cli_test.py": (
            "5b5a97a585e37f4583f6321935847acfc092d84dd8f1f8d194c3df30f4408554",
            frozenset({"4b033395eee9281e1672acdd2498354fb1061d2c1be3759b125e468526681e52"})),
        "scripts/release/verify_html_performance_evidence.py": (
            "73ffb34fc4fd5d46a698059879e29753f546c10f53815b20df5d7a9d17468060",
            frozenset({"486f9d60ffe64f6bb674a534ca26b070f145d80beb3a51e6060bc47d995b83f8",
                       "befc62abf92871676e8402337c8b8aae2ce209b373a10cba64e218fd07a889d9"})),
    }
    historic = historical_pairs.get(path)
    if historic is not None and hashlib.sha256(old).hexdigest() == historic[0]:
        if hashlib.sha256(current).hexdigest() in historic[1]:
            return True
    expected = _SAME_BINARY_REVIEWED_FILES.get(path)
    return (expected is not None and hashlib.sha256(old).hexdigest() == expected[0]
            and hashlib.sha256(current).hexdigest() == expected[1])


def reviewed_svg_provenance_change(measured: Records, current: Records,
                                   old_modes: Mapping[str, str], new_modes: Mapping[str, str]) -> bool:
    """既存の由来 gate で固定する二つの vendor 接続だけを性能再利用に認める。"""
    prefix = "crates/katana-render-runtime/vendor/svg-engine/"
    changes = {
        "PROVENANCE.json": (
            ("8cd06cd2533360467d8f90cf2de422747927d3fb54a2dd41e85a6018f1d4905f", 28884),
            ("07c779270618a9c2d0133367f247abf09bf2227b83b8af8bca3e64bc1fa9e145", 28884)),
        "usvg/writer.rs": (
            ("9ab4bff1772c728af183b659831dc381d49ff8e9a1c4cee223730b0ff8f3eb78", 60372),
            ("c73e39790272815346f2640bf582e17e0768db002da2b54f0c74f8c6e19f88ec", 60490)),
        "resvg/filter/component_transfer.rs": (
            ("5e9e6ed542fc9988a0060e7c642592797b6b04b8846f57aacfcac13c2d6d69cf", 2249),
            ("734564bd09c8a77fae2f04104bcaeee6117e329ccb8bfa3040ae91829500857b", 2373)),
    }
    anchors = {
        "README.md": ("0eb3c8eca056cf62fd51cda3471ca73bdf6d8e50bd6e989523d895d9c58e6a23", 7958),
        "LICENSE-APACHE": ("a6cba85bc92e0cff7a450b1d873c0eaa2e9fc96bf472df0247a26bec77bf3ff9", 10173),
        "LICENSE-MIT": ("f5d934dc281b44e0003ee461ac740b18b6629a454decd872c774d34e4ee0b21d", 1057),
    }
    old_vendor = {path for path in measured if path.startswith(prefix)}
    new_vendor = {path for path in current if path.startswith(prefix)}
    if old_vendor != new_vendor:
        return False
    for path in old_vendor:
        if old_modes.get(path) != "100644" or new_modes.get(path) != "100644":
            return False
        name = path[len(prefix):]
        if name in changes:
            if (measured.get(path), current.get(path)) != changes[name]:
                return False
        elif measured.get(path) != current.get(path):
            return False
    if not all(measured.get(prefix + name) == current.get(prefix + name) == record
               for name, record in anchors.items()):
        return False
    if not all(prefix + name in old_vendor for name in changes):
        return False
    program = "scripts/runtime-assets/svg-engine-provenance.ts"
    return (measured.get(program), current.get(program), old_modes.get(program), new_modes.get(program)) == (
        ("1c08a971ff2e98f5a99c805846bfefc3a11acc1dd6db900a216bff1a37a8d4ea", 5233),
        ("f57f6244c0b4557592b0a2b2eaca20ca0906d27deff4c20796e6025b9def57a5", 5233),
        "100644", "100644")


@dataclass(frozen=True)
class SourceReuseContext:
    measured_head: str
    eligible_head: str
    measured_records: Mapping[str, tuple[str, int]]
    changed_paths: tuple[str, ...]
    artifact_equivalence: bool = False
    program_equivalence: bool = False


def _modes(raw: bytes) -> dict[str, str]:
    result: dict[str, str] = {}
    for entry in raw.split(b"\0"):
        if not entry:
            continue
        metadata, name = entry.split(b"\t", 1)
        mode, kind, _oid = metadata.decode("ascii").split()
        path = name.decode("utf8")
        if kind != "blob" or mode not in ("100644", "100755") or path in result:
            raise ValueError("unsupported source tree entry")
        result[path] = mode
    return result


def _reviewed_parse5_cohort(
    root: Path, measured_head: str, current: Records, measured: Records,
    old_modes: Mapping[str, str], new_modes: Mapping[str, str], git_bytes: GitBytes,
) -> bool:
    if measured_head != SAME_BINARY_REVIEWED_BASE:
        return False
    pairs = {
        "package.json": (
            ("e084ea89276814cbe65f383c5d76a5d1896b6cae76f869dc8aa3de05cf6cdc61", 2306),
            ("dc042ef81b6d708539359e9fa7febd3588eaa81ad3cb9cd8a584216520c105b3", 2306)),
        "bun.lock": (
            ("a158d17634338c8bb8d90e535488f1f437bd6c4be7927ac09518f030a319aca3", 37784),
            ("a718edcc3070d497cc7243d143fe48e4d92db256dcda13fdc7a4d781ea58f470", 37784)),
    }
    replacements = {
        "package.json": ((b'"parse5": "8.1.0"', b'"parse5": "8.1.1"'),),
        "bun.lock": (
            (b'"parse5": "8.1.0"', b'"parse5": "8.1.1"'),
            (b'"parse5@8.1.0", "", { "dependencies": { "entities": "^9.0.0" } }, '
             b'"sha512-sq3qBFpee5jeocbFbO48VHlGr8sT7mbXS0vxlKN6sottBtJzrOG3998sr0llSZcQAGgfpdmEYAIZE0P9z0dE8w=="',
             b'"parse5@8.1.1", "", { "dependencies": { "entities": "^9.0.0" } }, '
             b'"sha512-EcR/6WDcU7b2N3vQSxKALCk9lXGtUMQxXio008OEMBE0kavu/7JLfBEeFPzi43nMqVDSbEn3QCK+BMCc9zO//w=="'),
        ),
    }
    for path, (old_record, new_record) in pairs.items():
        if (measured.get(path) != old_record or current.get(path) != new_record
                or old_modes.get(path) != "100644" or new_modes.get(path) != "100644"):
            return False
        old = git_bytes(("show", measured_head + ":" + path))
        new = (root / path).read_bytes()
        if ((hashlib.sha256(old).hexdigest(), len(old)) != old_record
                or (hashlib.sha256(new).hexdigest(), len(new)) != new_record):
            return False
        expected = old
        for previous, replacement in replacements[path]:
            if expected.count(previous) != 1:
                return False
            expected = expected.replace(previous, replacement, 1)
        if expected != new:
            return False
    return True


def _specific_source_changes(
    root: Path, measured_head: str, current: Records, measured: Records,
    old_modes: Mapping[str, str], new_modes: Mapping[str, str], git_bytes: GitBytes,
    *, allow_test_changes: bool, same_binary_rust: bool = False,
) -> tuple[set[str], set[str]]:
    """今回のHTML test、release文書と補助ツールの厳密な差分だけを認証する。"""
    parent = "crates/katana-render-runtime/src/renderer/backends/html_interactive/session.rs"
    child = "crates/katana-render-runtime/src/renderer/backends/html_interactive/session_resource_reuse_tests.rs"
    document = "docs/release.md"
    approved: set[str] = set()
    added_tests: set[str] = set()
    if same_binary_rust and _reviewed_parse5_cohort(
        root, measured_head, current, measured, old_modes, new_modes, git_bytes,
    ):
        approved.update(("package.json", "bun.lock"))

    release_tooling_pairs = {
        "biome.json": (
            "05c126306e2458e596d5f79357439885f9800d58123a047ed6b706cd8a7c75e3",
            "cf4f78df9d8b5516556d008bd7abaafc57d9c5c66b50bf884023652a3c3aa420"),
        ".github/workflows/release-preflight.yml": (
            "5addb675cf64e90845f02afd1795277f4469e1e1f8e39921f5672064d83f25c7",
            "416042cb077298c23965f1e3cb135188a21f90c60ce3f7477c198a2236c396d2"),
        ".github/workflows/release.yml": (
            "712905ef42152c05d2f5046d7f0efaee6af49323ac0c1e27c75255ce50896375",
            "d51fb8971a8e8b3dc7daa4827e4fcdf0152f5eb65eac245355d80458c123d93b"),
        ".github/workflows/test-and-build.yml": (
            "a2d9d28ee271d4ee29ff89514848be5134bbb27f809e19b7faf472111a09869b",
            "704c54e3656210ed96791ab28f996ca4cf2c2299c0591082fb7ee812e6d0de73"),
        "package.json": (
            "0bc2e819ab7d2522fd59e836b7fddd97b430884da9ded25d467543e60d241107",
            "2bd43597a3245b76613897ec56dd187c4afc38dc4f9ecaef58db675bfdbddcfa"),
        "bun.lock": (
            "15cb452be9d9b0788eb887b22bd8cf7065800166e6c2f32efac5fe48986fb482",
            "a62ffe885b5e436571de07badb32cd6cee71d034bd2d5b37809d3f2e10a0c6f7"),
    }
    for path, (expected_old, expected_new) in release_tooling_pairs.items():
        if path in approved:
            continue
        if measured.get(path) == current.get(path):
            continue
        if (path not in measured or path not in current or old_modes.get(path) != "100644"
                or new_modes.get(path) != "100644"):
            raise ValueError("release tooling membership or mode differs")
        old_bytes = git_bytes(("show", measured_head + ":" + path))
        new_bytes = (root / path).read_bytes()
        if (hashlib.sha256(old_bytes).hexdigest() != expected_old
                or hashlib.sha256(new_bytes).hexdigest() != expected_new
                or measured[path] != (expected_old, len(old_bytes))
                or current[path] != (expected_new, len(new_bytes))):
            raise ValueError("release tooling source delta differs")
        approved.add(path)

    if measured.get(document) != current.get(document):
        if document not in measured or document not in current:
            raise ValueError("release document membership differs")
        old_document = git_bytes(("show", measured_head + ":" + document))
        new_document = (root / document).read_bytes()
        removed = b'- \x60Test and Build (macos-15-intel, mac-x64)\x60\n'
        old_sha = hashlib.sha256(old_document).hexdigest()
        new_sha = hashlib.sha256(new_document).hexdigest()
        if (old_sha != "1a740b364aaf0ebacd59d1969d3a73313547917a9223a8d96c4c9531900542ab"
                or measured[document] != (old_sha, len(old_document))
                or current[document] != (new_sha, len(new_document))
                or old_modes.get(document) != "100644" or new_modes.get(document) != "100644"
                or old_document.count(removed) != 1
                or old_document.replace(removed, b"", 1) != new_document):
            raise ValueError("release document delta differs")
        approved.add(document)

    if not same_binary_rust and (measured.get(parent) != current.get(parent) or measured.get(child) != current.get(child)):
        if (not allow_test_changes or child in measured or child not in current
                or parent not in measured or parent not in current
                or old_modes.get(parent) != "100644" or new_modes.get(parent) != "100644"
                or child in old_modes or new_modes.get(child) != "100644"):
            raise ValueError("HTML test membership or mode differs")
        old_parent = git_bytes(("show", measured_head + ":" + parent))
        new_parent = (root / parent).read_bytes()
        test_bytes = (root / child).read_bytes()
        suffix = b'\n#[cfg(test)]\n#[path = "session_resource_reuse_tests.rs"]\nmod resource_reuse_tests;\n'
        old_sha = hashlib.sha256(old_parent).hexdigest()
        new_sha = hashlib.sha256(new_parent).hexdigest()
        test_sha = hashlib.sha256(test_bytes).hexdigest()
        if (old_sha != "d48356aab76a79a8e634395775d5deb2da5178a2f63a108c6c33164371615548"
                or measured[parent] != (old_sha, len(old_parent))
                or current[parent] != (new_sha, len(new_parent))
                or new_parent != old_parent + suffix
                or test_sha != "f2d210b9e8c536aad4ba67f05f23a5a9d79b3caab08ce56d2417cb960da93227"
                or current[child] != (test_sha, len(test_bytes))):
            raise ValueError("HTML test source delta differs")
        approved.update((parent, child))
        added_tests.add(child)

    test_cohort = {
        "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_coverage_gap_arrow_tests.rs": (("6accdb5675d801c5b26d5f144756c80d5a0ce15fde9dc31c89dd932e2d3a8611", 4255), ("3cfc61c620818207153699b67c177aa270fa87e333ba83bcb44eff4052baf5ec", 5179)),
        "crates/katana-render-runtime/src/markdown/svg_rasterize_html_glyph_instances_tests.rs": (("aa7efda0ad82341e40b2fd8f386916c87efa6511c8378f0afa8a126b14589d11", 10669), ("6abe3020035675c95be3c11bbd2d094a0535a881de3acafdbd3bf7bde8dae536", 11310)),
        "crates/katana-render-runtime/src/markdown/svg_rasterize_text_shaping_svg_fragments_tests.rs": (("daf7b57a647213588f12531521e6f13de146e859a6973be9c46867562dd559e6", 8606), ("c60f5c8df144249ed10a9e22fcd9e79d26d24dae1699b94a5a42ba1e26cb9462", 8717)),
        "crates/katana-render-runtime/src/svg_engine_parser_boundary_tests.rs": (("9f92c836a164c23be101fefe4aa16eec851b941f486b81f73b09c001f6fb3742", 11833), ("05f6eff97360095a517e5102c89a76cdcabdd38edba797dbc7c7837abcb2742b", 11914)),
        "crates/katana-render-runtime/src/svg_engine_parser_paint_boundary_tests.rs": (("3aa06c65e76258fcd704218f632dc185cb488a2c4c09a4c47c555feedcd0b7a3", 6757), ("9ebad52e95ecb48ddd1d07c332312e683b00bc460c6929fda271d287c25cbe51", 12022)),
        "crates/katana-render-runtime/src/svg_engine_parser_shape_boundary_tests.rs": (None, ("34729a9401f9f618227a09752c0f5ba7423d3405a315b840bcbd5f491d39c877", 1683)),
        "crates/katana-render-runtime/src/svg_engine_render_compare_tests.rs": (("28cc1bf2fe975adf992ad3496fc97a79a869dd754f5a06768161683f8ba9b77d", 3214), ("20272f7c034583f7de1fed81809c1bcc2aedf716c1f75a165e9677e6114c736e", 3225)),
        "crates/katana-render-runtime/src/svg_engine_render_filter_coverage_tests.rs": (None, ("fac834f41746e67736ddf014eb0457ad00c8b4292185a924ec1956a41a1c883b", 3043)),
        "crates/katana-render-runtime/src/svg_engine_render_filter_shadow_tests.rs": (("de49038437dc0c3ba5fac3fb6f4d268a9ef3d97a3d1e503d761d221cefcffae9", 2099), ("0da7255e3c460c47da541411010c8979bee21ba0f36e7bcd4573cde9360a3663", 2175)),
        "crates/katana-render-runtime/src/svg_engine_render_marker_tests.rs": (("cfd2d5f82082d5940c55eef4961f8232504bae18245f64d1c421b65520d5d29f", 6695), ("f1a2e1aaaf7afb0c3d4eabf53d903f818a0ac1685f7d53cd68cb9923bc02e078", 8718)),
        "crates/katana-render-runtime/src/svg_engine_text_layout_spacing_tests.rs": (("1cca57085e42db7a2076285740acbdb4be4bd4e9cd516074c7d2fb38bf9fd4be", 7970), ("1e609ade23cb2add14450419b36dce7829581d8251e323d8bebd3ec1fa65def1", 9219)),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/backend_route_edge_mutations_tests.rs": (None, ("bac5187c3d7baa9529901f89e2f32f04fc2b2b373993983cc93e4eaa4cceca65", 1325)),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/backend_route_tests.rs": (("a84dd8898ab9fa7c8ab28d230bf56e64824867d61a06d11cf76cebce520a6a02", 10068), ("8e09b1cd07dbcbe085ac37030d9ec882e87c1884578c6966441201550e678b6b", 9931)),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/bundle_execution_tests.rs": (("51feaea679e8d6011d0dd8ff5d7252a39ae0eb886283b19f4e393633f610e641", 10404), ("222c96eab71e037c0919a479a0b16ccbf92b9cda36154981f8c3e2e62b1618a6", 10665)),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/mathjax_backend_shape_mutations_tests.rs": (None, ("71ca42940ad7ab3347b006fe02e5c900f2d6205bd126d6844c53c2d917cb373f", 1856)),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/mermaid_classic_resource_edge_mutations_tests.rs": (None, ("44d702e0537bd5fd810c9a7a0092833a1ef098d9642e79775afc8fc31189e756", 1699)),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/mermaid_classic_resource_tests.rs": (("985354b8899dc7fa19e74078a8bca05db06c953f7b8b450f71b5eac119cc2eb2", 10622), ("6daf1f2289f7437a34bb5dd18a5e69f5c25c76cab6decceefdbfb44e0c6c5518", 10478)),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/mermaid_render_shape_mutations_tests.rs": (None, ("346faa7c7c1cf26be5b7e466f0f83e4facce7c0cc9386485bd7505724a538522", 1777)),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/resource_snapshot_shape_mutations_tests.rs": (None, ("7cc85e6690a4a0d0e74eda026dc3d176c71c9192970e85a3f51d2c39b1ebc139", 1445)),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/runtime_v8_route_plantuml_tests.rs": (("a4b7157ec64e3b736440a07c37ed82bd8d9e02f399c665f8c3e728e793c489b8", 6464), ("c88e9f369ec3f0a8a01c3e1d402770fb32834d9875fba20ad17d322000944082", 7916)),
    }
    test_anchors = {
        "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime.rs": ("6beaa9a5f1b2d52c87d4e34ab55e78ee2a00b57c8a992da3082ab5459300b28a", 4480),
        "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_tests.rs": ("91b414ac482e6f543dee80bd1828f0174bc313d31852cc24906a3b2fd2739ec7", 4658),
        "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_tests_block_geometry.rs": ("d27f32df71cbb09a3bd63daf2d8ab281b99d1edecd06218c87e5b82ce857b322", 2445),
        "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime_coverage_gap_tests.rs": ("7451561b72037373d8aebbb7b1b285a4dad4f261cdaf7337ba7c836a247836b1", 138),
        "crates/katana-render-runtime/src/markdown/svg_rasterize_html_glyph_instances.rs": ("b9fb49bdc1b6b8ec02537a4ca441bae92956d105fd68992fd883627c8f578230", 5923),
        "crates/katana-render-runtime/src/markdown/svg_rasterize_text_shaping_svg_fragments.rs": ("752c3d7daff11b21bdb5f3fd99d836772f377d0ef6abe4b46f155bf8ba290e90", 5381),
        "crates/katana-render-runtime/src/svg_engine.rs": ("c18945e539f8e6176c435ae0104b96d9b964b544d63afcdbff0d96fbb2c4bd39", 1952),
        "crates/katana-render-runtime/src/svg_engine_render_tests.rs": ("74dc29569c589fac46caa3f560d660d182c852921328d69f1eb3219a936c8040", 7098),
        "crates/katana-render-runtime/src/svg_engine_text_model_tests.rs": ("acebe5d6cbac8e9fdda49d5fc9035a11bcc47628c677a4255deccf8c95c279e3", 8403),
        "crates/katana-render-runtime/src/svg_engine_text_layout_matrix_tests.rs": ("78dd5c2a5c4fcad32db5deb9df82b186b439b30cfd7def83230025a359a6aaa1", 9114),
        "crates/katana-render-runtime/vendor/svg-engine/usvg/tree/text.rs": ("7b48f425e37fd5fa71d336f85ee0be6df0b1222143f08b18c57c10a3f2b5f307", 18938),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts.rs": ("e5fd686cbf0035422d1e8a95dfe9e7b5af990b3e58d824a32caf9224a85fc43f", 6391),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/ast_linter_tests.rs": ("b3832f508a3b27c4de35ac15ffd87b3dfe9ab9fd4f312749783a7d3a641f4618", 2926),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/mermaid_classic_scripts_tests.rs": ("6303a96003a43f287579e0a4ec07f1258780aa9f62f6a18434163f101e3fe735", 9863),
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/runtime_v8_route_tests.rs": ("1b11e1d66ae62827b6b676446ae8d5c593b01a08d7fa9b82ceb24c16af652ecd", 10381),
    }
    sentinels = {
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/backend_route_edge_mutations_tests.rs",
        "crates/kdr-linter/src/bin/mathjax_consumer_facts/mermaid_classic_resource_edge_mutations_tests.rs",
    }
    if not same_binary_rust and any(path not in measured and path in current for path in sentinels):
        if not allow_test_changes:
            raise ValueError("test-only cohort is not enabled")
        for path, (before, after) in test_cohort.items():
            if (measured.get(path) != before or current.get(path) != after
                    or old_modes.get(path) != ("100644" if before is not None else None)
                    or new_modes.get(path) != "100644"):
                raise ValueError("test-only cohort membership, mode or record differs")
            data = (root / path).read_bytes()
            if (hashlib.sha256(data).hexdigest(), len(data)) != after:
                raise ValueError("test-only cohort current bytes differ")
            if before is not None:
                data = git_bytes(("show", measured_head + ":" + path))
                if (hashlib.sha256(data).hexdigest(), len(data)) != before:
                    raise ValueError("test-only cohort measured bytes differ")
        for path, expected in test_anchors.items():
            if (measured.get(path) != expected or current.get(path) != expected
                    or old_modes.get(path) != "100644" or new_modes.get(path) != "100644"):
                raise ValueError("test-only cohort anchor differs")
            old_data = git_bytes(("show", measured_head + ":" + path))
            new_data = (root / path).read_bytes()
            if ((hashlib.sha256(old_data).hexdigest(), len(old_data)) != expected
                    or (hashlib.sha256(new_data).hexdigest(), len(new_data)) != expected):
                raise ValueError("test-only cohort anchor bytes differ")
        approved.update(test_cohort)
        added_tests.update(path for path, (before, _) in test_cohort.items() if before is None)

    return approved, added_tests


def validate_reuse_source(root: Path, measured_head: str, current: Records,
                          measured: Records, git_text: GitText,
                          git_bytes: GitBytes, *, allow_test_changes: bool = False,
                          program_records: Mapping[str, tuple[str, int]] | None = None,
                          program_modes: Mapping[str, str] | None = None,
                          same_binary_rust: bool = False,
                          allow_zero_scroll_reassessment: bool = False) -> SourceReuseContext:
    eligible = git_text(("rev-parse", "HEAD")).strip()
    if git_text(("merge-base", measured_head, eligible)).strip() != measured_head:
        raise ValueError("measured HEAD is not an ancestor")
    if git_text(("status", "--porcelain=v1", "--untracked-files=all")):
        raise ValueError("release source is dirty or untracked")
    old_modes = _modes(git_bytes(("ls-tree", "-r", "-z", measured_head)))
    new_modes = _modes(git_bytes(("ls-tree", "-r", "-z", eligible)))
    if (program_records is None) != (program_modes is None):
        raise ValueError("program source proof is incomplete")
    def is_program(path: str) -> bool:
        return path.startswith("scripts/") or path == "Justfile"
    if program_records is not None and program_modes is not None:
        if (not equivalent_program_records(
                {path: value for path, value in current.items() if is_program(path)}, program_records)
                or {path: value for path, value in new_modes.items() if is_program(path)} != program_modes):
            raise ValueError("current measurement program differs from authenticated program")
    parse5_cohort = same_binary_rust and _reviewed_parse5_cohort(
        root, measured_head, current, measured, old_modes, new_modes, git_bytes,
    )
    if same_binary_rust and any(
        path.rsplit("/", 1)[-1] in {"Cargo.lock", "bun.lock", "bun.lockb",
                                    "package-lock.json", "pnpm-lock.yaml", "yarn.lock"}
        and measured.get(path) != current.get(path)
        and not (path == "bun.lock" and parse5_cohort)
        for path in set(measured) | set(current)
    ):
        raise ValueError("lockfile differs")
    provenance = "crates/katana-render-runtime/vendor/svg-engine/PROVENANCE.json"
    metadata_changes: set[str] = set()
    provenance_program = "scripts/runtime-assets/svg-engine-provenance.ts"
    projected_program = ("f57f6244c0b4557592b0a2b2eaca20ca0906d27deff4c20796e6025b9def57a5", 5233)
    provenance_changed = measured.get(provenance) != current.get(provenance)
    vendor_prefix = "crates/katana-render-runtime/vendor/svg-engine/"
    vendor_rust_changed = any(
        path.startswith(vendor_prefix) and path.endswith(".rs")
        and measured.get(path) != current.get(path)
        for path in set(measured) | set(current)
    )
    if same_binary_rust and (provenance_changed or vendor_rust_changed
                             or current.get(provenance_program) == projected_program):
        if not reviewed_svg_provenance_change(measured, current, old_modes, new_modes):
            raise ValueError("SVG engine provenance cohort differs")
        if provenance_changed:
            metadata_changes.add(provenance)
    approved_changes, new_test_files = _specific_source_changes(
        root, measured_head, current, measured, old_modes, new_modes, git_bytes,
        allow_test_changes=allow_test_changes, same_binary_rust=same_binary_rust,
    )
    reassessment_changes = (_zero_scroll_policy_changes(
        measured_head, current, measured, old_modes, new_modes,
    ) if allow_zero_scroll_reassessment else set())
    def binary_rust_path(path: str) -> bool:
        parts = path.split("/")
        product = len(parts) >= 4 and parts[0] == "crates" and parts[2] == "src"
        vendor = (len(parts) >= 6 and parts[:4] == [
            "crates", "katana-render-runtime", "vendor", "svg-engine",
        ] and parts[4] in {"usvg", "resvg"})
        return (product or vendor) and path.endswith(".rs") and parts[-1] != "build.rs"
    rust_changes = {path for path in set(current) | set(measured)
                    if same_binary_rust and binary_rust_path(path)
                    and (current.get(path) != measured.get(path)
                         or old_modes.get(path) != new_modes.get(path))}
    if any(mode != "100644" for path in rust_changes
           for mode in (old_modes.get(path), new_modes.get(path)) if mode is not None):
        raise ValueError("changed Rust source mode differs")
    old_checked = {path: mode for path, mode in old_modes.items()
                   if (program_records is None or not is_program(path)) and path not in rust_changes}
    new_checked = {path: mode for path, mode in new_modes.items()
                   if (program_records is None or not is_program(path)) and path not in new_test_files
                   and path not in rust_changes}
    measured_checked = {path for path in measured
                        if (program_records is None or not is_program(path)) and path not in rust_changes}
    current_checked = {path for path in current
                       if (program_records is None or not is_program(path)) and path not in new_test_files
                       and path not in rust_changes}
    if (old_checked != new_checked or current_checked != measured_checked
            or set(current) != set(new_modes) or set(measured) != set(old_modes)):
        raise ValueError("source membership or mode differs")
    changed = tuple(sorted(path for path in set(current) | set(measured)
                           if current.get(path) != measured.get(path)
                           or old_modes.get(path) != new_modes.get(path)))
    program_changes = reassessment_changes | {
        path for path in changed if program_records is not None and is_program(path)
    }
    test_changes = set(changed) - ALLOWED - program_changes - approved_changes - rust_changes - metadata_changes
    if test_changes:
        if not allow_test_changes or not test_changes <= TEST_PATHS | {TEST_PARENT}:
            raise ValueError("source changes exceed documentation allowance")
        # WHY: 事前にcfg(test)接続を確認したsource以外は候補にしない。
        parent = (root / TEST_PARENT).read_text(encoding="utf8")
        previous = git_text(("show", measured_head + ":" + TEST_PARENT))
        previous_sha = hashlib.sha256(previous.encode()).hexdigest()
        if (previous_sha not in TEST_PARENT_SHA
                or measured.get(TEST_PARENT, (None,))[0] != previous_sha):
            raise ValueError("test-only parent baseline differs")
        if TEST_PARENT in test_changes:
            if parent.count(TEST_LOGGER) != 1 or parent.replace(TEST_LOGGER, "", 1) != previous:
                raise ValueError("test-only parent delta differs")
        if any(measured.get(path, (None,))[0] != checksum
               or current.get(path, (None,))[0] != checksum
               for path, checksum in TEST_ANCHOR_SHA.items()):
            raise ValueError("test-only connection baseline differs")
    if any(old_modes.get(path) != "100644" for path in changed
           if path not in program_changes and path not in new_test_files and path not in rust_changes):
        raise ValueError("changed source is not a regular non-executable file")
    for name, (checksum, size) in current.items():
        path = root / name
        if (not path.is_file() or path.is_symlink() or path.resolve() != path
                or path.stat().st_size != size
                or hashlib.sha256(path.read_bytes()).hexdigest() != checksum):
            raise ValueError("working source differs from current committed tree: " + name)
    return SourceReuseContext(measured_head, eligible, dict(measured), changed,
                              bool(test_changes or new_test_files or rust_changes), bool(program_changes))

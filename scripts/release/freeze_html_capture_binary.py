"""既存の固定コピー処理をHTML取得の実行ファイルへ適用する。"""

from __future__ import annotations

import argparse
from collections.abc import Sequence
import json
from pathlib import Path
from typing import cast

from produce_html_performance_evidence import freeze_binary


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args(arguments)
    source, destination = cast(Path, args.source), cast(Path, args.destination)
    checksum = freeze_binary(source, destination)
    print(json.dumps({"source": str(source), "path": str(destination),
                      "sha256": checksum}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

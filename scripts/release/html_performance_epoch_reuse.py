"""既に認証されたepoch入力の宣言を比較する。"""
from __future__ import annotations

from collections.abc import Mapping
from typing import TypedDict

import verify_html_performance_evidence as gate


class DeclaredEpochInputs(TypedDict):
    workers: Mapping[str, gate.JsonObject]
    programs: Mapping[str, gate.JsonObject]
    inputs: Mapping[str, gate.JsonObject]
    resources: Mapping[str, gate.JsonObject]
    fonts: Mapping[str, gate.JsonObject]
    tool_context: gate.JsonObject


def _verified_pins(reader: gate.FileReader, values: Mapping[str, gate.JsonObject],
                   label: str) -> dict[str, tuple[str, str]]:
    result: dict[str, tuple[str, str]] = {}
    for role, ref in values.items():
        gate.require(isinstance(role, str) and bool(role), label + " role is invalid")
        pin = gate._object(ref, label + " pin")
        gate.require(set(pin) == {"path", "sha256"}, label + " pin fields differ")
        path = gate._text(pin.get("path"), label + " path")
        checksum = gate._text(pin.get("sha256"), label + " SHA")
        gate.digest(checksum)
        gate.require(gate.sha256(reader.file(pin)) == checksum, label + " bytes differ")
        result[role] = (path, checksum)
    return result


def compare_declared_inputs(old_reader: gate.FileReader, old: DeclaredEpochInputs,
                            new_reader: gate.FileReader, new: DeclaredEpochInputs) -> None:
    """宣言済み入力だけを比較する。宣言の完全性や測定適格性は呼出側が検証する。"""
    for epoch in (old, new):
        gate.require(set(epoch["workers"]) == {"baseline", "candidate"},
                     "epoch worker roles differ")
        gate.require(bool(epoch["programs"]) and bool(epoch["inputs"]),
                     "epoch program or input inventory is empty")
        context = epoch["tool_context"]
        gate.require(all(name in context for name in ("environmentSha256", "platform", "cpuCount")),
                     "epoch tool context is incomplete")
        gate.digest(context["environmentSha256"])
        gate.require(bool(gate._text(context["platform"], "epoch platform"))
                     and gate._integer(context["cpuCount"], "epoch CPU count") > 0,
                     "epoch platform or CPU count is invalid")
    groups = (("workers", old["workers"], new["workers"]),
              ("programs", old["programs"], new["programs"]),
              ("inputs", old["inputs"], new["inputs"]),
              ("resources", old["resources"], new["resources"]),
              ("fonts", old["fonts"], new["fonts"]))
    for name, old_group, new_group in groups:
        old_pins = _verified_pins(old_reader, old_group, "old " + name)
        new_pins = _verified_pins(new_reader, new_group, "new " + name)
        gate.require(old_pins == new_pins, "epoch " + name + " differs")
    gate.require(old["tool_context"] == new["tool_context"], "epoch tool context differs")

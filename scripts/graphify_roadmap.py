#!/usr/bin/env python3
"""Overlay explicit, source-backed roadmap traceability onto a Graphify AST graph.

Only allowlisted Markdown is read. Task checkboxes remain document assertions, not
proof of device acceptance. Curated code/test bindings indicate relevant evidence,
not that any requirement is fully satisfied or a test was executed by this script.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile

ORIGIN = "ctdozlite-roadmap-v1"
ID = r"(?:[A-Z]+-)?[A-Z]+-\d+"
TASK = re.compile(r"^- \[([ xX])\] \[([A-Z]+-T\d+)\](.*)$")
REQUIREMENT = re.compile(r"^\| (" + ID + r") \| (.+?) \|$")


def read_source(root: Path, relative: str) -> str:
    """Read a repository Markdown source without permitting path escape."""
    path = (root / relative).resolve()
    if not path.is_relative_to(root.resolve()) or path.suffix != ".md":
        raise ValueError(f"Not an allowed Markdown source: {relative}")
    return path.read_text(encoding="utf-8")


def atomic_write(path: Path, text: str) -> None:
    """Replace an output only after the complete new content is ready."""
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=".roadmap-", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(text)
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def enrich(root: Path, graph: dict, manifest: dict) -> tuple[dict, dict]:
    """Return an idempotent overlay, validating all explicit bindings first."""
    if manifest.get("schema_version") != 1:
        raise ValueError("Unsupported roadmap manifest version")
    graph = json.loads(json.dumps(graph))
    old = {n["id"] for n in graph["nodes"] if n.get("_origin") == ORIGIN}
    graph["nodes"] = [n for n in graph["nodes"] if n["id"] not in old]
    graph["links"] = [e for e in graph["links"] if e.get("_origin") != ORIGIN
                      and e["source"] not in old and e["target"] not in old]
    nodes = {n["id"]: n for n in graph["nodes"]}
    stats = {"documents": 0, "requirements": 0, "tasks": 0, "open_tasks": 0,
             "test_bindings": 0, "releases": {}, "source_hashes": {}}
    edges = set()

    def node(key: str, label: str, kind: str, source: str, line: int = 1, **attrs) -> str:
        ident = "roadmap:" + key
        if ident not in nodes:
            item = dict(id=ident, label=label, kind=kind, file_type="document",
                        source_file=source, source_location=f"L{line}",
                        _origin=ORIGIN, **attrs)
            nodes[ident] = item
            graph["nodes"].append(item)
        return ident

    def edge(start: str, end: str, relation: str, source: str, line: int = 1) -> None:
        if start not in nodes or end not in nodes:
            raise ValueError(f"Dangling binding: {start} -> {end}")
        signature = (start, end, relation)
        if signature not in edges:
            edges.add(signature)
            graph["links"].append(dict(source=start, target=end, relation=relation,
                                      confidence="EXTRACTED", confidence_score=1.0,
                                      weight=1.0, source_file=source,
                                      source_location=f"L{line}", _origin=ORIGIN))

    # Keep existing AST symbol identity so document-to-code navigation works.
    def code_file(path: str) -> str:
        if not (root / path).is_file() or not path.startswith(("src/", "scripts/", "tests/")):
            raise ValueError(f"Missing/unsupported code binding: {path}")
        existing = [n for n in graph["nodes"] if n.get("source_file") == path
                    and n.get("label") == Path(path).name]
        return existing[0]["id"] if existing else node("file:" + path, path, "code_file", path)

    docs = {manifest["roadmap"], manifest["gap_source"], *manifest["evidence"]}
    for release in manifest["releases"]:
        docs.add(f"docs/releases/{release}-plan.md")
    docs.add("docs/releases/v0.008-arm-progress.md")
    spec_names = set(manifest["carryover_specs"])
    for info in manifest["releases"].values():
        spec_names.update(info["specs"])
    for name in spec_names:
        for filename in ("spec.md", "plan.md", "tasks.md", "verification.md"):
            docs.add(f"specs/{name}/{filename}")
    contents = {path: read_source(root, path) for path in sorted(docs)}
    requirements = {}
    tasks = []
    for path, text in contents.items():
        stats["source_hashes"][path] = hashlib.sha256(text.encode()).hexdigest()
        title = next((line.lstrip("# ") for line in text.splitlines() if line.startswith("# ")), path)
        doc = node("doc:" + path, title, "document", path)
        stats["documents"] += 1
        lines = text.splitlines()
        headings = [(i, line.lstrip("# ")) for i, line in enumerate(lines) if line.startswith("#")]
        for index, (offset, heading) in enumerate(headings):
            stop = headings[index + 1][0] if index + 1 < len(headings) else len(lines)
            if index == 0 and heading == title:
                continue
            section = node(f"section:{path}:{offset + 1}", heading, "document_section", path,
                           offset + 1, description="\n".join(lines[offset + 1:stop]))
            edge(doc, section, "contains", path, offset + 1)
        if path.startswith("specs/"):
            spec = path.split("/")[1]
            for lineno, line in enumerate(lines, 1):
                match = REQUIREMENT.match(line) if path.endswith("/spec.md") else None
                if match:
                    req_id, description = match.groups()
                    req = node(f"requirement:{spec}:{req_id}", f"{req_id} — {description}",
                               "requirement", path, lineno, requirement_id=req_id,
                               spec=spec, status="see_tasks_and_device_acceptance")
                    requirements[(spec, req_id)] = req
                    edge(doc, req, "defines", path, lineno)
                    stats["requirements"] += 1
                match = TASK.match(line) if path.endswith("/tasks.md") else None
                if match:
                    checked, task_id, description = match.groups()
                    status = "document_checked" if checked.lower() == "x" else "open"
                    task = node(f"task:{spec}:{task_id}", f"{task_id} — {description.strip()}",
                                "task", path, lineno, task_id=task_id, status=status,
                                checkbox_checked=status == "document_checked")
                    tasks.append((spec, task, description, path, lineno))
                    edge(doc, task, "contains", path, lineno)
                    stats["tasks"] += 1
                    stats["open_tasks"] += status == "open"
    # Reference ranges such as S-01…S-06 remain within their spec namespace.
    for spec, task, description, path, lineno in tasks:
        for (req_spec, req_id), req in requirements.items():
            if spec != req_spec:
                continue
            referenced = req_id in re.findall(ID, description)
            for prefix, start, end in re.findall(r"([A-Z]+)-(\d+)…\1-(\d+)", description):
                if req_id.startswith(prefix + "-"):
                    referenced |= int(start) <= int(req_id.split("-")[-1]) <= int(end)
            if referenced:
                edge(task, req, "addresses", path, lineno)
        for code in sorted(set(re.findall(r"(?:src|tests|scripts)/[A-Za-z0-9_./-]+\.(?:rs|py|sh|ps1)", description))):
            if (root / code).is_file():
                edge(task, code_file(code), "mentions_code", path, lineno)
    # Resolve only local Markdown links already present in the allowlist.
    for path, text in contents.items():
        for match in re.finditer(r"\[[^\]]+\]\(([^)]+)\)", text):
            target = match.group(1).split("#", 1)[0]
            resolved = (root / path).parent.joinpath(target).resolve()
            if resolved.is_relative_to(root.resolve()):
                relative = resolved.relative_to(root.resolve()).as_posix()
                if relative in contents:
                    edge("roadmap:doc:" + path, "roadmap:doc:" + relative, "references",
                         path, text.count("\n", 0, match.start()) + 1)
    source = manifest["gap_source"]
    gaps = {}
    for lineno, line in enumerate(contents[source].splitlines(), 1):
        match = re.match(r"\| ([AB]\d{2}) \| ([^|]+) \|", line)
        if not match:
            match = re.match(r"- (C\d{2}) — (.*)", line)
        if match:
            gap_id, label = match.groups()
            gaps[gap_id] = node("gap:" + gap_id, f"{gap_id} — {label.strip()}", "gap",
                                source, lineno, status="historical_gap_see_current_specs")
            edge("roadmap:doc:" + source, gaps[gap_id], "defines", source, lineno)
    expected = {f"A{i:02}" for i in range(1, 21)} | {f"B{i:02}" for i in range(1, 13)} | {f"C{i:02}" for i in range(1, 5)}
    assigned = {gap for values in manifest["blocks"].values() for gap in values}
    if set(gaps) != expected or assigned != expected:
        raise ValueError("Roadmap must preserve all A01–A20, B01–B12, C01–C04")
    roadmap = manifest["roadmap"]
    for release, info in manifest["releases"].items():
        plan = f"docs/releases/{release}-plan.md"
        release_node = node("release:" + release, release, "release", plan, status=info["status"])
        edge("roadmap:doc:" + roadmap, release_node, "plans", roadmap)
        edge(release_node, "roadmap:doc:" + plan, "documented_by", plan)
        stats["releases"][release] = info
        for block in info["blocks"]:
            block_node = node("block:" + block, block, "work_block", roadmap)
            edge(release_node, block_node, "includes", roadmap)
            for gap_id in manifest["blocks"][block]:
                edge(block_node, gaps[gap_id], "scopes", roadmap)
        for spec in info["specs"]:
            edge(release_node, "roadmap:doc:specs/" + spec + "/spec.md", "specifies", plan)
    for spec, blocks in manifest.get("spec_blocks", {}).items():
        for block in blocks:
            edge("roadmap:block:" + block, "roadmap:doc:specs/" + spec + "/spec.md",
                 "specifies", roadmap)
    for dependency in manifest.get("dependencies", []):
        edge("roadmap:block:" + dependency["from"], "roadmap:block:" + dependency["to"],
             dependency["relation"], roadmap)
    for release in manifest["releases"]:
        edge("roadmap:release:" + release, gaps["C04"], "compatibility_crosscut", roadmap)
    for spec, block in manifest["carryover_specs"].items():
        edge("roadmap:block:" + block, "roadmap:doc:specs/" + spec + "/spec.md",
             "carries_open_work_from", roadmap)
    # These bindings are reviewed assertions in the manifest, not LLM guesses.
    manifest_source = "tools/graphify/roadmap.json"
    for binding in manifest["bindings"]:
        for req_id in binding["requirements"]:
            req = requirements[(binding["spec"], req_id)]
            for code in binding["code"]:
                edge(req, code_file(code), "implementation_context", manifest_source)
            for reference in binding["tests"]:
                path, name = reference.split("::", 1)
                symbols = [n for n in graph["nodes"] if n.get("source_file") == path
                           and n.get("label") == name + "()" and n.get("_origin") != ORIGIN]
                if len(symbols) != 1:
                    raise ValueError(f"Missing/ambiguous AST test binding: {reference}")
                rust = (root / path).read_text(encoding="utf-8")
                if not re.search(r"#\[test\]\s*fn\s+" + re.escape(name) + r"\b", rust):
                    raise ValueError(f"Binding is not a unit test: {reference}")
                edge(req, symbols[0]["id"], "has_regression_source", manifest_source)
                stats["test_bindings"] += 1
    for gap_id, (spec, *req_ids) in manifest.get("gap_bindings", {}).items():
        for req_id in req_ids:
            edge(gaps[gap_id], requirements[(spec, req_id)], "related_requirement", manifest_source)
    stats["nodes"] = sum(n.get("_origin") == ORIGIN for n in graph["nodes"])
    stats["edges"] = sum(e.get("_origin") == ORIGIN for e in graph["links"])
    graph["graph"]["roadmap_overlay"] = stats
    return graph, stats


def report(graph: dict, stats: dict) -> str:
    """Render a readable status index without inventing completion metrics."""
    lines = ["# Roadmap trong Graphify", "", "Sinh từ tài liệu và bindings có nguồn; không phải nghiệm thu thiết bị.",
             "Checkbox checked chỉ là trạng thái task trong tài liệu. Cạnh test chỉ xác nhận mã test tồn tại, không xác nhận lần chạy.", "",
             f"{stats['documents']} tài liệu; {stats['requirements']} yêu cầu; {stats['tasks']} task ({stats['open_tasks']} còn mở).", "",
             "| Phiên bản | Khối | Trạng thái |", "| :--- | :--- | :--- |"]
    for release, info in stats["releases"].items():
        lines.append(f"| {release} | {', '.join(info['blocks'])} | {info['status']} |")
    lines += ["", "## Yêu cầu → mã/test → task còn mở", "",
              "Đây là bindings có chọn lọc; test chưa được script này thực thi. Ô trống không kết luận không có mã/test.", "",
              "| Spec / yêu cầu | Mã liên quan | Test source | Task mở |",
              "| :--- | :--- | :--- | :--- |"]
    nodes = {n["id"]: n for n in graph["nodes"]}
    for req in graph["nodes"]:
        if req.get("_origin") != ORIGIN or req.get("kind") != "requirement":
            continue
        code, tests, pending = set(), set(), set()
        for link in graph["links"]:
            if link["source"] == req["id"]:
                target = nodes[link["target"]]
                if link["relation"] == "implementation_context":
                    code.add(target["source_file"])
                elif link["relation"] == "has_regression_source":
                    tests.add(target["label"])
            if link["target"] == req["id"] and link["relation"] == "addresses":
                task = nodes[link["source"]]
                if task.get("status") == "open":
                    pending.add(task["task_id"])
                for context in graph["links"]:
                    if context["source"] == task["id"] and context["relation"] == "mentions_code":
                        code.add(nodes[context["target"]]["source_file"])
        lines.append(f"| {req['spec']} / {req['requirement_id']} | {'; '.join(sorted(code)) or '—'} | {'; '.join(sorted(tests)) or '—'} | {', '.join(sorted(pending)) or 'xem verification'} |")
    lines += ["", "## Task chưa đóng / còn cần kiểm chứng", ""]
    for node in graph["nodes"]:
        if node.get("_origin") == ORIGIN and node.get("kind") == "task" and node.get("status") == "open":
            lines.append(f"- {node['label']} ({node['source_file']}:{node['source_location']})")
    lines += ["", "## Giới hạn truy vết", "", "Các nhóm A/B là baseline v0.007, không phải trạng thái hiện tại.",
              "v0.009–v0.011 vẫn là kế hoạch; có mã nền không chứng minh phạm vi mới đã triển khai.",
              "Đọc verification/progress và nguồn PC/Web trước khi kết luận tương thích.",
              "Bindings có chọn lọc; thiếu cạnh test không đồng nghĩa không có test.", ""]
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--ast-graph", type=Path, default=Path("graphify-out/ast/graphify-out/graph.json"))
    args = parser.parse_args()
    root = args.root.resolve()
    manifest = json.loads((root / "tools/graphify/roadmap.json").read_text())
    output = root / "graphify-out"
    graph, stats = enrich(root, json.loads((root / args.ast_graph).read_text()), manifest)
    atomic_write(output / "graph.json", json.dumps(graph, ensure_ascii=False, indent=2) + "\n")
    atomic_write(output / "ROADMAP_REPORT.md", report(graph, stats))
    atomic_write(output / "ROADMAP_INDEX.json", json.dumps(stats, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({k: v for k, v in stats.items() if k not in ("source_hashes", "releases")}, ensure_ascii=False))


if __name__ == "__main__":
    main()

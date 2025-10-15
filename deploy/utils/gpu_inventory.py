# SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import argparse
import json
import os
import re
import shutil
import sys
import subprocess
from dataclasses import asdict, dataclass
from typing import Dict, List, Optional, Tuple, Union

from kubernetes import client, config

def run_command(cmd: List[str], capture_output: bool = True, exit_on_error: bool = True):  # type: ignore
    try:
        return subprocess.run(
            cmd, capture_output=capture_output, text=True, check=True
        )
    except subprocess.CalledProcessError as e:  # pragma: no cover - passthrough
        if exit_on_error:
            print(f"ERROR: Command failed: {' '.join(cmd)}", file=sys.stderr)
            if e.stdout:
                print(e.stdout, file=sys.stderr)
            if e.stderr:
                print(e.stderr, file=sys.stderr)
            sys.exit(e.returncode)
        raise


NVIDIA_PREFIX = "nvidia.com/"
LABEL_GPU_COUNT = f"{NVIDIA_PREFIX}gpu.count"
LABEL_GPU_PRODUCT = f"{NVIDIA_PREFIX}gpu.product"
LABEL_GPU_MEMORY = f"{NVIDIA_PREFIX}gpu.memory"  # MiB per GPU
LABEL_MIG_CAPABLE = f"{NVIDIA_PREFIX}mig.capable"


@dataclass
class NodeGpuInventory:
    node_name: str
    gpu_count: Optional[int]
    gpu_product: Optional[str]
    gpu_memory_mib: Optional[int]
    mig_capable: Optional[bool]
    allocatable_gpu: Optional[int]
    mig_resources: Dict[str, str]

    def to_dict(self) -> Dict[str, Union[str, int, bool, Dict[str, str], None]]:
        return asdict(self)


def _parse_int(value: Optional[str]) -> Optional[int]:
    if value is None:
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        match = re.search(r"\d+", str(value))
        return int(match.group(0)) if match else None


def _bool_from_str(value: Optional[str]) -> Optional[bool]:
    if value is None:
        return None
    s = str(value).strip().lower()
    if s in {"true", "1", "yes"}:
        return True
    if s in {"false", "0", "no"}:
        return False
    return None


def _normalize_node(node: Union[client.V1Node, Dict]) -> Dict:
    # Convert V1Node to dict for uniform access
    if hasattr(node, "to_dict"):
        return node.to_dict()
    return node  # assume already dict


def _extract_inventory(node_obj: Dict) -> NodeGpuInventory:
    meta = node_obj.get("metadata", {})
    status = node_obj.get("status", {})
    labels = meta.get("labels", {}) or {}

    node_name = meta.get("name", "<unknown>")
    gpu_product = labels.get(LABEL_GPU_PRODUCT)
    gpu_memory_mib = _parse_int(labels.get(LABEL_GPU_MEMORY))
    mig_capable = _bool_from_str(labels.get(LABEL_MIG_CAPABLE))

    # Prefer GFD-reported GPU count if present; otherwise use allocatable nvidia.com/gpu
    gpu_count = _parse_int(labels.get(LABEL_GPU_COUNT))

    alloc = status.get("allocatable", {}) or {}
    alloc_gpu = _parse_int(alloc.get(f"{NVIDIA_PREFIX}gpu"))

    if gpu_count is None:
        gpu_count = alloc_gpu

    # Collect MIG resource keys and counts if present
    mig_resources: Dict[str, str] = {
        k: str(v)
        for k, v in alloc.items()
        if isinstance(k, str) and k.startswith(f"{NVIDIA_PREFIX}mig-") and _parse_int(str(v))
    }

    return NodeGpuInventory(
        node_name=node_name,
        gpu_count=gpu_count,
        gpu_product=gpu_product,
        gpu_memory_mib=gpu_memory_mib,
        mig_capable=mig_capable,
        allocatable_gpu=alloc_gpu,
        mig_resources=mig_resources,
    )


def _list_nodes_via_client() -> List[Dict]:
    # Assume running inside a Kubernetes pod with service account
    try:
        config.load_incluster_config()
    except Exception as e:
        raise RuntimeError(
            f"Failed to load in-cluster Kubernetes config. Ensure this runs in a pod with a service account. Error: {e}"
        )

    v1 = client.CoreV1Api()
    items = v1.list_node().items  # type: ignore[attr-defined]
    return [_normalize_node(n) for n in items]


def _list_nodes_via_kubectl() -> List[Dict]:
    if not shutil.which("kubectl"):
        raise RuntimeError("kubectl not found in PATH for fallback")
    result = run_command(["kubectl", "get", "nodes", "-o", "json"], capture_output=True)
    data = json.loads(result.stdout)
    return data.get("items", [])


def collect_gpu_inventory(prefer_client: bool = True) -> Tuple[List[NodeGpuInventory], str]:
    sources_tried: List[str] = []
    errors: List[str] = []

    def _via_client() -> List[NodeGpuInventory]:
        items = _list_nodes_via_client()
        return [_extract_inventory(n) for n in items]

    def _via_kubectl() -> List[NodeGpuInventory]:
        items = _list_nodes_via_kubectl()
        return [_extract_inventory(n) for n in items]

    if prefer_client:
        try:
            sources_tried.append("kubernetes-client")
            return _via_client(), ",".join(sources_tried)
        except Exception as e:
            errors.append(str(e))
            try:
                sources_tried.append("kubectl-json")
                return _via_kubectl(), ",".join(sources_tried)
            except Exception as e2:
                errors.append(str(e2))
                raise RuntimeError("Failed to list nodes: " + " | ".join(errors))
    else:
        try:
            sources_tried.append("kubectl-json")
            return _via_kubectl(), ",".join(sources_tried)
        except Exception as e:
            errors.append(str(e))
            try:
                sources_tried.append("kubernetes-client")
                return _via_client(), ",".join(sources_tried)
            except Exception as e2:
                errors.append(str(e2))
                raise RuntimeError("Failed to list nodes: " + " | ".join(errors))


def _format_gib(mib: Optional[int]) -> str:
    if mib is None:
        return ""
    return f"{mib/1024:.1f} GiB"


def print_table(rows: List[NodeGpuInventory], show_mig: bool = False) -> None:
    headers = ["NODE", "GPUS", "MODEL", "VRAM/GPU", "MIG"]
    table: List[List[str]] = []
    for r in rows:
        mig_str = ""
        if r.mig_capable is True:
            if r.mig_resources:
                mig_str = ",".join(f"{k.split('/')[-1]}={v}" for k, v in sorted(r.mig_resources.items()))
            else:
                mig_str = "capable"
        elif r.mig_capable is False:
            mig_str = "no"

        table.append(
            [
                r.node_name,
                "" if r.gpu_count is None else str(r.gpu_count),
                r.gpu_product or "",
                _format_gib(r.gpu_memory_mib),
                mig_str if show_mig else ("yes" if r.mig_capable else ""),
            ]
        )

    # Compute column widths
    widths = [len(h) for h in headers]
    for row in table:
        for i, cell in enumerate(row):
            widths[i] = max(widths[i], len(cell))

    def _fmt_row(row: List[str]) -> str:
        return "  ".join(cell.ljust(widths[i]) for i, cell in enumerate(row))

    print(_fmt_row(headers))
    print(_fmt_row(["-" * w for w in widths]))
    for row in table:
        print(_fmt_row(row))


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Report GPU inventory per Kubernetes node (count, SKU, VRAM)."
    )
    parser.add_argument(
        "--format",
        "-o",
        choices=["table", "json"],
        default="table",
        help="Output format",
    )
    parser.add_argument(
        "--prefer",
        choices=["client", "kubectl"],
        default="client",
        help="Prefer Kubernetes Python client or kubectl JSON fallback",
    )
    parser.add_argument(
        "--show-mig",
        action="store_true",
        help="In table output, show MIG resource types and counts",
    )

    args = parser.parse_args()

    prefer_client = args.prefer == "client"
    rows, source = collect_gpu_inventory(prefer_client=prefer_client)

    if args.format == "json":
        payload = {
            "source": source,
            "items": [r.to_dict() for r in rows],
        }
        print(json.dumps(payload, indent=2))
        return

    print_table(rows, show_mig=args.show_mig)


if __name__ == "__main__":
    main()



# Final Documentation Structure

**Branch**: `docs-reorg`
**Date**: October 15, 2025

---

## Complete Structure

```
docs/
├── api/                      (11 docs) [RENAMED from API/]
│   └── nixl_connect/         NIXL API reference
│
├── architecture/             (10 docs) [STREAMLINED - removed kvbm, planner]
│   ├── architecture.md
│   ├── disagg_serving.md
│   ├── distributed_runtime.md
│   ├── dynamo_flow.md
│   ├── kv_cache_routing.md
│   ├── request_cancellation.md
│   └── request_migration.md
│
├── backends/                 (23 docs) [UNCHANGED]
│   ├── sglang/               9 backend-specific guides
│   ├── trtllm/               10 backend-specific guides
│   └── vllm/                 5 backend-specific guides
│
├── benchmarks/               (2 docs) [UNCHANGED]
│   ├── benchmarking.md
│   └── pre_deployment_profiling.md
│
├── development/              (2 docs) [NEW]
│   ├── backend-guide.md      How to write backends
│   └── runtime-guide.md      Runtime development
│
├── examples/                 (2 docs) [UNCHANGED]
│   └── runtime/              Runtime examples
│
├── guides/                   (1 doc) [STREAMLINED]
│   └── tool-calling.md       Genuine how-to guide
│
├── images/                   [UNCHANGED]
│   └── ... (architecture diagrams, screenshots)
│
├── kubernetes/               (14 docs) [UNCHANGED - kept at root]
│   ├── README.md
│   ├── installation_guide.md
│   ├── dynamo_operator.md
│   ├── create_deployment.md
│   ├── multinode-deployment.md
│   ├── sla_planner_quickstart.md
│   ├── metrics.md
│   ├── logging.md
│   ├── api_reference.md
│   ├── fluxcd.md
│   ├── grove.md
│   ├── model_caching_with_fluid.md
│   ├── gke_setup.md
│   └── minikube.md
│
├── kvbm/                     (7 docs) [NEW - consolidated from multiple places]
│   ├── kvbm_intro.rst        Entry point
│   ├── kvbm_architecture.md  Architecture overview
│   ├── kvbm_components.md    Component details
│   ├── kvbm_motivation.md    Why KVBM exists
│   ├── kvbm_reading.md       Further reading
│   ├── vllm-setup.md         vLLM setup guide
│   └── trtllm-setup.md       TensorRT-LLM setup guide
│
├── observability/            (3 docs) [NEW]
│   ├── health-checks.md      Health monitoring
│   ├── logging.md            Logging setup
│   └── metrics.md            Metrics collection
│
├── performance/              (1 doc) [NEW]
│   └── tuning.md             Performance tuning guide
│
├── planner/                  (3 docs) [NEW]
│   ├── planner_intro.rst     Entry point
│   ├── sla_planner.md        SLA-based planning
│   └── load_planner.md       Load-based planning
│
├── reference/                (3 docs) [NEW]
│   ├── cli.md                CLI reference
│   ├── glossary.md           Terminology
│   └── support-matrix.md     Platform compatibility
│
├── router/                   (1 doc) [MOVED from components/]
│   └── README.md             Router documentation
│
├── conf.py                   Sphinx configuration
├── hidden_toctree.rst        Hidden pages
├── index.rst                 Main entry point
├── Makefile                  Build commands
├── README.md                 Build instructions
└── generate_docs.py          Doc generation script
```

---

## Summary by Category

### Core System Components (Top-Level)
- **kvbm/** - KV Block Manager (7 docs)
- **planner/** - Request planning (3 docs)
- **router/** - Request routing (1 doc)

### User-Facing Content
- **observability/** - Monitoring & debugging (3 docs)
- **performance/** - Optimization guides (1 doc)
- **guides/** - How-to guides (1 doc)

### Developer Resources
- **development/** - Developer guides (2 docs)
- **reference/** - Reference material (3 docs)

### Deployment
- **kubernetes/** - K8s deployment (14 docs)
- **backends/** - Backend-specific (23 docs)

### Foundation
- **architecture/** - Core concepts (10 docs)
- **api/** - API reference (11 docs)

---

## Changes from Original

### Directories Added (6)
1. `kvbm/` - Consolidated KVBM content
2. `planner/` - Consolidated planner content
3. `observability/` - Monitoring & debugging
4. `performance/` - Performance guides
5. `development/` - Developer guides
6. `reference/` - Reference material

### Directories Removed (3)
1. `components/` - Content moved to top level
2. `deploy/` - Was empty except symlink
3. `runtime/` - Moved to development/
4. Most of `guides/` - Distributed to proper sections

### Directories Renamed (1)
1. `API/` → `api/` (lowercase consistency)

### Directories Unchanged (6)
1. `architecture/` (streamlined)
2. `backends/`
3. `benchmarks/`
4. `examples/`
5. `images/`
6. `kubernetes/` (as requested)

---

## Files Moved

### To kvbm/ (7 files)
- `architecture/kvbm_*.md` (5 files) → `kvbm/`
- `backends/vllm/kvbm-setup.md` → `kvbm/vllm-setup.md`
- `backends/trtllm/kvbm-setup.md` → `kvbm/trtllm-setup.md`

### To planner/ (3 files)
- `architecture/planner_intro.rst` → `planner/`
- `architecture/sla_planner.md` → `planner/`
- `architecture/load_planner.md` → `planner/`

### To observability/ (3 files)
- `guides/health_check.md` → `observability/health-checks.md`
- `guides/logging.md` → `observability/logging.md`
- `guides/metrics.md` → `observability/metrics.md`

### To development/ (2 files)
- `guides/backend.md` → `development/backend-guide.md`
- `runtime/README.md` → `development/runtime-guide.md`

### To performance/ (1 file)
- `guides/disagg_perf_tuning.md` → `performance/tuning.md`

### To reference/ (3 files)
- `guides/dynamo_run.md` → `reference/cli.md`
- `dynamo_glossary.md` (root) → `reference/glossary.md`
- `support_matrix.md` (root) → `reference/support-matrix.md`

### To router/ (1 file)
- `components/router/README.md` → `router/README.md`

---

## Benefits

✅ **Component Discovery** - kvbm, planner, router at top level (easy to find)
✅ **Logical Grouping** - Related content together
✅ **Consistent Naming** - All lowercase
✅ **Scalable Structure** - Clear place for new content
✅ **Clean Architecture** - architecture/ now focused on core concepts
✅ **No Duplication** - Single source of truth

---

## Total Impact

- **45 files moved/renamed**
- **11 new directories created**
- **4 directories removed**
- **~80 internal links updated**
- **6 commits** with clear history

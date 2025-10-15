# Dynamo Documentation Structure Analysis

**Date**: 2025-10-15
**Branch**: main (commit: a7badb855)
**Status**: Clean working tree

---

## Current Structure (ACTUAL)

```
docs/
├── _build/                   # Sphinx build artifacts
├── _extensions/              # Custom Sphinx extensions
├── _includes/                # Reusable RST fragments
├── _sections/                # Navigation section RST files
├── _static/                  # CSS, JS, images
│
├── API/                      (11 docs) - NIXL API reference
├── architecture/             (15 docs) ✅ Core architecture concepts
├── backends/                 (23 docs) ✅ vLLM, SGLang, TRT-LLM guides
│   ├── sglang/               (9 docs)
│   ├── trtllm/               (10 docs + multinode/)
│   └── vllm/                 (5 docs)
├── benchmarks/               (2 docs) - Benchmarking & profiling guides
├── components/               (1 doc) - Only has router/
│   └── router/               (1 README.md)
├── deploy/                   (0 docs) - Has metrics/ subdir only
│   └── metrics/              (README + images)
├── examples/                 (2 docs) - Runtime examples
│   └── runtime/              hello_world example
├── guides/                   (9 docs) - Mixed how-to guides
├── images/                   (0 docs) - Shared image assets
├── kubernetes/               (14 docs) ⚠️ Should be in deployment/
├── runtime/                  (1 doc) - Runtime development guide
│
├── conf.py                   # Sphinx configuration
├── dynamo_glossary.md        # Glossary (root level)
├── hidden_toctree.rst        # Hidden pages navigation
├── index.rst                 # Main entry point
├── README.md                 # Build instructions
├── support_matrix.md         # Platform support matrix
├── Makefile                  # Documentation build commands
├── generate_docs.py          # Doc generation script
└── exclusions.txt            # Doc exclusion patterns
```

---

## Issues Identified

### 1. **Inconsistent Naming**
- `API/` is uppercase while everything else is lowercase
- Should be: `api/` for consistency

### 2. **Scattered Kubernetes Documentation**
- `kubernetes/` (14 docs) exists at root level
- Should be organized under a `deployment/` hierarchy
- Links in `index.rst` use `../kubernetes/` paths

### 3. **Thin Components Directory**
- `components/` only has `router/` with 1 README
- Missing: planner, kvbm, frontend, nixl docs
- Planner & KVBM docs are in `architecture/` instead

### 4. **Guides Directory Catch-All**
Contains 9 mixed guides that belong in specific sections:
- `backend.md` - belongs in development/
- `disagg_perf_tuning.md` - performance tuning
- `dynamo_run.md` - CLI/tooling reference
- `health_check.md` - observability
- `logging.md` - observability
- `metrics.md` - observability
- `run_kvbm_in_trtllm.md` - backend-specific
- `run_kvbm_in_vllm.md` - backend-specific
- `tool_calling.md` - how-to guide

### 5. **Empty/Unclear Directories**
- `deploy/` has no docs at top level, only `metrics/` subdir
- `examples/` unclear if this belongs in `/docs` or root `/examples`

### 6. **Duplicate Root Files**
- `dynamo_glossary.md` at root vs potential `reference/glossary.md`
- `support_matrix.md` at root vs potential `reference/support-matrix.md`

### 7. **Missing Documentation Sections**
No organized sections for:
- Installation (scattered in guides)
- Deployment (kubernetes at root, no local/cloud/bare-metal)
- Observability (guides in multiple places)
- Reference (API, CLI, glossary all scattered)
- Troubleshooting (no dedicated section)
- Performance (tuning guides scattered)

---

## Proposed Improvements

### Option A: Minimal Reorganization (Conservative)
**Goal**: Fix the most glaring issues without major restructure

1. Rename `API/` → `api/`
2. Create `deployment/kubernetes/` and move 14 kubernetes docs
3. Distribute `guides/` content to appropriate sections
4. Create `reference/` directory for glossary + support matrix
5. Update `index.rst` navigation

**Pros**: Low risk, minimal changes
**Cons**: Still somewhat disorganized, doesn't solve all issues

---

### Option B: Comprehensive Reorganization (Recommended)
**Goal**: Create a logical, scalable structure

```
docs/
├── _build/, _extensions/, _includes/, _sections/, _static/  [Keep]
│
├── getting-started/          [New] First-time user guides
│   ├── quickstart.md
│   ├── installation.md
│   └── concepts.md
│
├── deployment/               [New] All deployment guides
│   ├── local/
│   │   └── quickstart.md
│   ├── kubernetes/           [Move from /kubernetes/]
│   │   ├── README.md
│   │   ├── installation_guide.md
│   │   ├── operator.md
│   │   └── ... (14 files)
│   └── cloud/
│       ├── aws.md
│       ├── gcp.md
│       └── azure.md
│
├── architecture/             [Keep] Core concepts
│
├── backends/                 [Keep] Backend-specific guides
│
├── components/               [Expand] Component documentation
│   ├── frontend/
│   ├── router/              [Keep existing]
│   ├── planner/             [Move from architecture/]
│   └── kvbm/                [Move from architecture/]
│
├── guides/                   [Reorganize] How-to guides only
│   ├── performance-tuning.md  [Consolidate perf guides]
│   ├── tool-calling.md
│   └── dynamo-run.md
│
├── observability/            [New] Monitoring & debugging
│   ├── metrics.md           [From guides/]
│   ├── logging.md           [From guides/]
│   ├── health-checks.md     [From guides/]
│   └── tracing.md
│
├── development/              [New] Developer guides
│   ├── backend-guide.md     [From guides/]
│   ├── runtime-guide.md     [From runtime/]
│   └── custom-worker.md
│
├── benchmarking/             [Rename from benchmarks/]
│   └── ... (keep existing)
│
├── reference/                [New] Reference documentation
│   ├── api/                 [Move from API/]
│   ├── cli.md               [From guides/dynamo_run.md]
│   ├── glossary.md          [From root]
│   └── support-matrix.md    [From root]
│
├── troubleshooting/          [New]
│   └── common-issues.md
│
├── images/                   [Keep]
│
├── conf.py, index.rst, README.md, etc.  [Keep, update links]
```

**Pros**: Clean, logical, scalable structure
**Cons**: More work, more link updates needed

---

### Option C: Incremental Approach (Hybrid)
**Goal**: Fix critical issues first, improve iteratively

**Phase 1** (Immediate):
1. Move `kubernetes/` → `deployment/kubernetes/`
2. Rename `API/` → `api/`
3. Move root `dynamo_glossary.md` and `support_matrix.md` to `reference/`
4. Update `index.rst` navigation

**Phase 2** (Near-term):
1. Organize `guides/` content
2. Create `observability/` section
3. Expand `components/` directory

**Phase 3** (Future):
1. Create `getting-started/` section
2. Add `development/` section
3. Improve cross-linking

**Pros**: Incremental value, lower risk each phase
**Cons**: Temporary inconsistency during transition

---

## Recommendation

**I recommend Option C (Incremental Approach)**

### Why?
1. **Lower risk**: Each phase is small and testable
2. **Immediate value**: Fix biggest pain points first (kubernetes at root)
3. **Flexibility**: Can adjust based on feedback
4. **Continuity**: Doesn't break existing links all at once

### Phase 1 Execution Plan
1. Create `deployment/` and `reference/` directories
2. Move 14 kubernetes docs → `deployment/kubernetes/`
3. Move 2 root files → `reference/`
4. Rename `API/` → `api/`
5. Update `index.rst` with new paths
6. Update internal links (automated script)
7. Test Sphinx build
8. Commit with clear description

**Estimated time**: 30-45 minutes
**Files affected**: ~30 files moved, ~50 links updated

---

## Next Steps

Please choose:
- **Option A**: Minimal (safest, least improvement)
- **Option B**: Comprehensive (most improvement, most work)
- **Option C**: Incremental (recommended balance)

Once you approve, I'll execute the chosen approach with proper git commits at each step.

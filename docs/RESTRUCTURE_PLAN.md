# Focused Documentation Restructure Plan

**Goal**: Fix real organizational issues without over-restructuring
**Principle**: Keep `kubernetes/` where it is (no premature `deployment/` hierarchy)

---

## Problems to Fix

### 1. **Inconsistent Naming: `API/` (uppercase)**
- Only directory with uppercase naming
- Should be `api/` for consistency

### 2. **Scattered `guides/` (9 files belonging elsewhere)**
Current guides and where they should go:

| Current File | Correct Location | Reason |
|-------------|------------------|---------|
| `guides/backend.md` | `development/backend-guide.md` | Developer guide for writing backends |
| `guides/disagg_perf_tuning.md` | `performance/tuning.md` | Performance optimization |
| `guides/dynamo_run.md` | `reference/cli.md` | CLI tool reference |
| `guides/health_check.md` | `observability/health-checks.md` | Monitoring/observability |
| `guides/logging.md` | `observability/logging.md` | Monitoring/observability |
| `guides/metrics.md` | `observability/metrics.md` | Monitoring/observability |
| `guides/run_kvbm_in_trtllm.md` | `backends/trtllm/kvbm.md` | Backend-specific feature |
| `guides/run_kvbm_in_vllm.md` | `backends/vllm/kvbm.md` | Backend-specific feature |
| `guides/tool_calling.md` | `guides/tool-calling.md` | Keep - genuine how-to guide |

### 3. **Duplicate/Scattered Observability Content**
Observability docs exist in multiple places:

| File | Location | Action |
|------|----------|--------|
| `guides/metrics.md` | General metrics guide | Move to `observability/` |
| `kubernetes/metrics.md` | K8s-specific metrics | Keep in kubernetes/ |
| `guides/logging.md` | General logging guide | Move to `observability/` |
| `kubernetes/logging.md` | K8s-specific logging | Keep in kubernetes/ |
| `guides/health_check.md` | Health checks guide | Move to `observability/` |

### 4. **Thin `components/` Directory**
- Only has `router/` with 1 README
- Missing: planner and kvbm docs (currently in `architecture/`)

### 5. **Empty `deploy/` Directory**
- Has no docs at top level
- Only contains `metrics/` subdir with symlink
- Should be removed from `/docs` (deployment artifacts in root `/deploy/`)

### 6. **Root-Level Reference Files**
- `dynamo_glossary.md` at root
- `support_matrix.md` at root
- Should be organized under `reference/`

---

## Proposed Changes

### **Phase 1: Rename & Consolidate** (30 min)

#### 1.1 Rename for consistency
```bash
mv docs/API docs/api
```

#### 1.2 Create new directories
```bash
mkdir -p docs/observability
mkdir -p docs/performance
mkdir -p docs/development
mkdir -p docs/reference
```

#### 1.3 Move observability content
```bash
mv docs/guides/health_check.md docs/observability/health-checks.md
mv docs/guides/logging.md docs/observability/logging.md
mv docs/guides/metrics.md docs/observability/metrics.md
```

#### 1.4 Move development content
```bash
mv docs/guides/backend.md docs/development/backend-guide.md
mv docs/runtime/README.md docs/development/runtime-guide.md
```

#### 1.5 Move performance content
```bash
mv docs/guides/disagg_perf_tuning.md docs/performance/tuning.md
```

#### 1.6 Move backend-specific guides
```bash
mv docs/guides/run_kvbm_in_vllm.md docs/backends/vllm/kvbm-setup.md
mv docs/guides/run_kvbm_in_trtllm.md docs/backends/trtllm/kvbm-setup.md
```

#### 1.7 Move reference content
```bash
mv docs/guides/dynamo_run.md docs/reference/cli.md
mv docs/dynamo_glossary.md docs/reference/glossary.md
mv docs/support_matrix.md docs/reference/support-matrix.md
```

#### 1.8 Keep useful guides
```bash
# Rename for consistency (kebab-case)
mv docs/guides/tool_calling.md docs/guides/tool-calling.md
```

#### 1.9 Clean up empty directories
```bash
rmdir docs/guides  # Will be empty after moves
rmdir docs/runtime  # Will be empty after moves
rm -rf docs/deploy  # Only has symlink, not useful in docs/
```

---

## Final Structure

```
docs/
├── _build/, _extensions/, _includes/, _sections/, _static/  [Unchanged]
│
├── api/                      [RENAMED from API/]
│   └── nixl_connect/         (11 docs)
│
├── architecture/             [Unchanged]
│   └── ... (15 docs)
│
├── backends/                 [Expanded]
│   ├── sglang/               (9 docs)
│   ├── trtllm/               (10 docs + kvbm-setup.md) ← NEW
│   └── vllm/                 (5 docs + kvbm-setup.md) ← NEW
│
├── benchmarks/               [Unchanged]
│   └── ... (2 docs)
│
├── components/               [Unchanged for now]
│   └── router/               (1 doc)
│
├── development/              [NEW]
│   ├── backend-guide.md      ← FROM guides/
│   └── runtime-guide.md      ← FROM runtime/
│
├── examples/                 [Unchanged]
│   └── runtime/              (2 docs)
│
├── guides/                   [Streamlined]
│   └── tool-calling.md       (1 doc - genuine how-to)
│
├── images/                   [Unchanged]
│
├── kubernetes/               [Unchanged] ✅
│   └── ... (14 docs)
│
├── observability/            [NEW]
│   ├── health-checks.md      ← FROM guides/
│   ├── logging.md            ← FROM guides/
│   └── metrics.md            ← FROM guides/
│
├── performance/              [NEW]
│   └── tuning.md             ← FROM guides/disagg_perf_tuning.md
│
├── reference/                [NEW]
│   ├── cli.md                ← FROM guides/dynamo_run.md
│   ├── glossary.md           ← FROM root dynamo_glossary.md
│   └── support-matrix.md     ← FROM root support_matrix.md
│
├── conf.py                   [Unchanged]
├── hidden_toctree.rst        [Update links]
├── index.rst                 [Update navigation]
├── Makefile                  [Unchanged]
├── README.md                 [Unchanged]
└── generate_docs.py          [Unchanged]
```

---

## Removed Directories
- `deploy/` - Only had symlink, not useful in docs
- `guides/` - Distributed to proper locations
- `runtime/` - Moved to development/

---

## Link Updates Required

### Files with Links to Update (~30 files):
1. `index.rst` - Main navigation
2. `hidden_toctree.rst` - Hidden page references
3. All files in `backends/` that reference guides
4. All files in `kubernetes/` that reference guides
5. Files that cross-reference glossary/support-matrix

### Link Update Strategy:
```bash
# Example: Update references to old guides/backend.md
find docs -name "*.md" -o -name "*.rst" | xargs sed -i '' 's|guides/backend\.md|development/backend-guide.md|g'

# Update API → api
find docs -name "*.md" -o -name "*.rst" | xargs sed -i '' 's|/API/|/api/|g'
find docs -name "*.md" -o -name "*.rst" | xargs sed -i '' 's|(API/|(api/|g'

# Update root reference files
find docs -name "*.md" -o -name "*.rst" | xargs sed -i '' 's|dynamo_glossary\.md|reference/glossary.md|g'
find docs -name "*.md" -o -name "*.rst" | xargs sed -i '' 's|support_matrix\.md|reference/support-matrix.md|g'
```

---

## Benefits

1. ✅ **Consistent naming**: All lowercase directories
2. ✅ **Logical organization**: Content grouped by purpose
3. ✅ **Clear observability section**: All monitoring docs together
4. ✅ **Better reference section**: CLI, glossary, support matrix organized
5. ✅ **Cleaner backends/**: Backend-specific features with backends
6. ✅ **No premature abstraction**: Keeping kubernetes/ at root (appropriate)
7. ✅ **Removed clutter**: Empty deploy/ and scattered guides/ gone

---

## Testing Checklist

- [ ] All files moved successfully (no lost content)
- [ ] Run Sphinx build: `cd docs && python generate_docs.py`
- [ ] No build warnings/errors
- [ ] Spot-check 5-10 key pages in browser
- [ ] Verify navigation works in built HTML
- [ ] Check links to moved pages

---

## Rollback Plan

All changes in git:
```bash
git add -A
git commit -m "docs: Restructure - organize guides, observability, and reference sections"

# If issues found:
git reset --hard HEAD~1
```

---

## Execution Time

- Directory moves: 5 min
- Link updates: 15 min
- Navigation updates: 5 min
- Testing: 10 min
- **Total: ~35 minutes**

---

## Next Steps

1. Review this plan
2. Approve to proceed
3. I'll execute with git commits at key checkpoints
4. Test build
5. Create summary of URL changes for external maintainers

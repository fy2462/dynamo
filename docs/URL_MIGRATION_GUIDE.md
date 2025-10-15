# Documentation URL Migration Guide

**Date**: October 15, 2025
**Branch**: docs-reorg

This document tracks all URL changes from the documentation reorganization for external maintainers and link references.

---

## Summary

The documentation has been reorganized to create logical sections and consistent naming. This affects approximately **24 file paths** that external documentation or bookmarks may reference.

---

## Directory Changes

### Renamed Directories

| Old Path | New Path | Reason |
|----------|----------|--------|
| `/docs/API/` | `/docs/api/` | Consistency (all lowercase) |

### New Directories Created

- `/docs/observability/` - Monitoring, logging, health checks
- `/docs/performance/` - Performance tuning guides
- `/docs/development/` - Developer guides
- `/docs/reference/` - CLI, glossary, support matrix

### Removed Directories

- `/docs/deploy/` - Empty except symlink
- `/docs/runtime/` - Content moved to development/
- Most of `/docs/guides/` - Content distributed to appropriate sections

---

## File-by-File URL Changes

### API Documentation
```
OLD: /docs/API/nixl_connect/*.md
NEW: /docs/api/nixl_connect/*.md
```
**Impact**: 11 files
**Action**: Update any links to use lowercase `api/`

---

### Observability & Monitoring

```
OLD: /docs/guides/health_check.md
NEW: /docs/observability/health-checks.md

OLD: /docs/guides/logging.md
NEW: /docs/observability/logging.md

OLD: /docs/guides/metrics.md
NEW: /docs/observability/metrics.md
```
**Impact**: 3 files moved, 1 renamed (kebab-case)
**Note**: Kubernetes-specific metrics/logging remain in `/docs/kubernetes/`

---

### Development Guides

```
OLD: /docs/guides/backend.md
NEW: /docs/development/backend-guide.md

OLD: /docs/runtime/README.md
NEW: /docs/development/runtime-guide.md
```
**Impact**: 2 files
**Audience**: Backend developers

---

### Performance Tuning

```
OLD: /docs/guides/disagg_perf_tuning.md
NEW: /docs/performance/tuning.md
```
**Impact**: 1 file
**Note**: Shorter, clearer name

---

### Backend-Specific Features

```
OLD: /docs/guides/run_kvbm_in_vllm.md
NEW: /docs/backends/vllm/kvbm-setup.md

OLD: /docs/guides/run_kvbm_in_trtllm.md
NEW: /docs/backends/trtllm/kvbm-setup.md
```
**Impact**: 2 files
**Reason**: Co-located with backend documentation

---

### Reference Documentation

```
OLD: /docs/guides/dynamo_run.md
NEW: /docs/reference/cli.md

OLD: /docs/dynamo_glossary.md  (root level)
NEW: /docs/reference/glossary.md

OLD: /docs/support_matrix.md  (root level)
NEW: /docs/reference/support-matrix.md
```
**Impact**: 3 files
**Note**: Clearer categorization as reference material

---

### How-To Guides (Unchanged Location)

```
OLD: /docs/guides/tool_calling.md
NEW: /docs/guides/tool-calling.md
```
**Impact**: 1 file (only renamed to kebab-case)
**Reason**: Genuine how-to guide, stays in guides/

---

## Navigation Changes

### Main Navigation (index.rst)

Updated paths in the main navigation:
- Support Matrix: `support_matrix.md` → `reference/support-matrix.md`
- Logging: `guides/logging.md` → `observability/logging.md`
- Health Checks: `guides/health_check.md` → `observability/health-checks.md`
- Performance Tuning: `guides/disagg_perf_tuning.md` → `performance/tuning.md`
- Backend Guide: `guides/backend.md` → `development/backend-guide.md`
- Glossary: `dynamo_glossary.md` → `reference/glossary.md`

### Hidden Pages (hidden_toctree.rst)

Updated references to:
- All `API/` → `api/` paths
- `runtime/README.md` → `development/runtime-guide.md`
- All moved guides files

---

## Quick Reference Table

| Old URL | New URL | Status |
|---------|---------|--------|
| `docs/API/*` | `docs/api/*` | ✅ Renamed |
| `docs/guides/backend.md` | `docs/development/backend-guide.md` | ✅ Moved |
| `docs/guides/health_check.md` | `docs/observability/health-checks.md` | ✅ Moved |
| `docs/guides/logging.md` | `docs/observability/logging.md` | ✅ Moved |
| `docs/guides/metrics.md` | `docs/observability/metrics.md` | ✅ Moved |
| `docs/guides/disagg_perf_tuning.md` | `docs/performance/tuning.md` | ✅ Moved |
| `docs/guides/run_kvbm_in_vllm.md` | `docs/backends/vllm/kvbm-setup.md` | ✅ Moved |
| `docs/guides/run_kvbm_in_trtllm.md` | `docs/backends/trtllm/kvbm-setup.md` | ✅ Moved |
| `docs/guides/dynamo_run.md` | `docs/reference/cli.md` | ✅ Moved |
| `docs/guides/tool_calling.md` | `docs/guides/tool-calling.md` | ✅ Renamed |
| `docs/dynamo_glossary.md` | `docs/reference/glossary.md` | ✅ Moved |
| `docs/support_matrix.md` | `docs/reference/support-matrix.md` | ✅ Moved |
| `docs/runtime/README.md` | `docs/development/runtime-guide.md` | ✅ Moved |

---

## For External Maintainers

### If You Maintain Links to Dynamo Docs

1. **Update bookmarks** to use new paths above
2. **Search your codebase** for references to moved files
3. **Check CI/CD pipelines** that may reference documentation paths
4. **Update any generated documentation** that links to Dynamo docs

### Automated Fix Script

```bash
# Example: Update all markdown files in your project
find . -name "*.md" -exec sed -i '' 's|docs/API/|docs/api/|g' {} \;
find . -name "*.md" -exec sed -i '' 's|guides/backend\.md|development/backend-guide.md|g' {} \;
find . -name "*.md" -exec sed -i '' 's|guides/health_check\.md|observability/health-checks.md|g' {} \;
# ... etc
```

---

## Unchanged Locations

These documentation areas were **NOT** changed:

✅ `/docs/architecture/` - All architecture docs remain
✅ `/docs/backends/` - Backend guides location unchanged (only added files)
✅ `/docs/benchmarks/` - Benchmarking guides unchanged
✅ `/docs/kubernetes/` - Kubernetes docs remain at root level
✅ `/docs/components/router/` - Component docs unchanged
✅ `/docs/examples/` - Examples unchanged

---

## Questions or Issues?

If you encounter broken links or have questions about the new structure:
1. Check this migration guide first
2. Search the new structure: https://github.com/ai-dynamo/dynamo/tree/docs-reorg/docs
3. Open an issue: https://github.com/ai-dynamo/dynamo/issues

---

## Internal Links

All internal documentation cross-references have been updated in this reorganization. If you find a broken internal link, please report it as a bug.

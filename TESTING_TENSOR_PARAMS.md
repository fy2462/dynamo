# Testing Tensor Parameters Feature

This document describes the test suite for the tensor parameters feature (GitHub Issue #3496).

## Overview

The feature adds parameter support to Dynamo's tensor model mode in the gRPC frontend, allowing KServe clients to send metadata alongside tensor data.

## Test Structure

### 1. **Rust Unit Test** (Comprehensive)

**Location:** `lib/llm/tests/kserve_service.rs::test_tensor_parameters`

**Scope:** Full end-to-end parameter flow within the Rust gRPC service layer

**What it tests:**
- ✅ Request-level parameters (all 5 types: bool, int64, string, double, uint64)
- ✅ Tensor-level input parameters
- ✅ Tensor-level output parameters
- ✅ Parameter type preservation through conversion
- ✅ KServe ↔ Dynamo conversion functions
- ✅ Worker can echo and add parameters

**How to run:**
```bash
cd lib/llm
cargo test --features integration test_tensor_parameters
```

**Test flow:**
```
gRPC Client (test)
    ↓ ModelInferRequest with parameters
KServe Service (conversion)
    ↓ NvCreateTensorRequest
ParameterEchoEngine (mock)
    ↓ Echoes + adds parameters
    ↓ NvCreateTensorResponse
KServe Service (conversion)
    ↓ ModelInferResponse
gRPC Client validates
```

**Assertions:**
- All input parameters present in response
- Parameter types preserved (int64→int64, string→string, etc.)
- Worker-added parameters present
- Tensor-level parameters work correctly

---

### 2. **Python Integration Test** (Realistic)

**Location:** `tests/frontend/test_grpc_tensor_parameters.py`

**Scope:** Full stack integration using tritonclient and real worker process

**What it tests:**
- ✅ Request-level parameters with real gRPC client (tritonclient)
- ✅ Output tensor parameters from Python worker
- ✅ Backward compatibility (requests without parameters)
- ✅ Real worker process over NATS

**How to run:**
```bash
# With pytest markers
pytest -xsv tests/frontend/test_grpc_tensor_parameters.py -m "integration and pre_merge"

# Or directly
pytest -xsv tests/frontend/test_grpc_tensor_parameters.py
```

**Test cases:**

1. **`test_grpc_tensor_request_level_parameters`**
   - Sends request with 4 different parameter types
   - Verifies all parameters echoed correctly
   - Verifies worker can add new parameters
   - Validates parameter type preservation

2. **`test_grpc_tensor_output_parameters`**
   - Tests output tensor-level parameters
   - Verifies worker can add tensor parameters

3. **`test_grpc_tensor_parameters_backward_compatible`**
   - Sends request WITHOUT parameters
   - Verifies system still works (no regression)
   - Ensures optional parameter support

**Test markers:**
```python
@pytest.mark.integration
@pytest.mark.pre_merge
@pytest.mark.gpu_0
```

**Fixtures used:**
- `etcd_server` - etcd instance from conftest
- `nats_server` - NATS instance from conftest
- `grpc_frontend` - Starts gRPC frontend on port 8000
- `tensor_worker` - Starts tensor_params_worker.py

**Test flow:**
```
pytest starts
    ↓
Fixtures: etcd + NATS + gRPC frontend + worker
    ↓
tritonclient.grpc.InferenceServerClient
    ↓ Real gRPC request
Dynamo gRPC Frontend (localhost:8000)
    ↓ Convert + route via NATS
tensor_params_worker.py
    ↓ Process + return
Dynamo gRPC Frontend
    ↓ Convert back
tritonclient receives response
    ↓
Assertions validate parameters
```

---

## Running the Tests

### Prerequisites

```bash
# Install Python dependencies
pip install tritonclient[grpc] pytest

# Start infrastructure (for manual testing)
docker compose -f deploy/docker-compose.yml up -d
```

### Rust Unit Test

```bash
cd lib/llm
cargo test --features integration test_tensor_parameters -- --nocapture
```

### Python Integration Test

```bash
# Run all parameter tests
pytest -xsv tests/frontend/test_grpc_tensor_parameters.py

# Run specific test
pytest -xsv tests/frontend/test_grpc_tensor_parameters.py::test_grpc_tensor_request_level_parameters

# With markers (as in CI)
pytest -xsv -m "integration and pre_merge and gpu_0" tests/frontend/test_grpc_tensor_parameters.py
```

### Manual Testing

For manual/exploratory testing, use the standalone worker:

```bash
# Terminal 1: Frontend
python -m dynamo.frontend --kserve-grpc-server --http-port 8000

# Terminal 2: Worker
python tensor_params_worker.py

# Terminal 3: Client
python test_tensor_params_client.py
```

---

## What Gets Validated

### Functional Requirements

| Requirement | Rust Test | Python Test |
|-------------|-----------|-------------|
| Request-level parameters flow | ✅ | ✅ |
| Tensor-level input parameters | ✅ | ⚠️ (not exposed by tritonclient) |
| Tensor-level output parameters | ✅ | ✅ |
| Parameter type preservation | ✅ | ✅ |
| All 5 parameter types work | ✅ | ✅ |
| Worker can add parameters | ✅ | ✅ |
| Backward compatibility | ⚠️ | ✅ |

### Non-Functional Requirements

| Requirement | Status |
|-------------|--------|
| No breaking changes | ✅ Optional fields |
| Serde serialization works | ✅ Automatic |
| NATS transport preserves params | ✅ JSON |
| KServe spec compliance | ✅ Correct protobuf |

---

## CI Integration

### Rust Tests

Runs in `.github/workflows/container-validation-dynamo.yml`:

```bash
cargo test --locked --features integration -- --nocapture
```

The `test_tensor_parameters` test will run as part of the integration test suite.

### Python Tests

Runs in pytest suite with markers:

```bash
pytest -m "integration and pre_merge and gpu_0"
```

The tests will be included in pre-merge CI validation.

---

## Test Files

- `lib/llm/tests/kserve_service.rs` - Rust unit test
- `tests/frontend/test_grpc_tensor_parameters.py` - Python integration test
- `tensor_params_worker.py` - Standalone worker for manual testing
- `test_tensor_params_client.py` - Standalone client for manual testing

---

## Success Criteria

Tests pass if:

1. ✅ Request parameters sent by client arrive at worker
2. ✅ Worker parameters sent back arrive at client
3. ✅ Parameter types are preserved (int64 stays int64, etc.)
4. ✅ All 5 parameter types work (bool, int64, string, double, uint64)
5. ✅ Tensor-level parameters work
6. ✅ Empty/missing parameters don't cause errors

---

## Cleanup

To remove temporary test files:

```bash
rm tensor_params_worker.py test_tensor_params_client.py TESTING_TENSOR_PARAMS.md
```

The formal tests in `lib/llm/tests/` and `tests/frontend/` should be kept as permanent test coverage.


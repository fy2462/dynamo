# SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

"""Integration test for gRPC frontend tensor parameters support."""

from __future__ import annotations

import logging
import shutil
import time

import numpy as np
import pytest

try:
    import tritonclient.grpc as grpcclient
except ImportError:
    grpcclient = None

from tests.conftest import EtcdServer, NatsServer
from tests.utils.constants import QWEN
from tests.utils.managed_process import ManagedProcess

logger = logging.getLogger(__name__)

TEST_MODEL = QWEN


def _extract_parameters(param_map) -> dict:
    """Extract parameter values from KServe InferParameter map.

    Args:
        param_map: Dictionary mapping parameter names to InferParameter objects

    Returns:
        Dictionary with parameter names mapped to their Python values
    """
    param_dict = {}
    for key, param in param_map.items():
        if param.HasField("bool_param"):
            param_dict[key] = param.bool_param
        elif param.HasField("int64_param"):
            param_dict[key] = param.int64_param
        elif param.HasField("string_param"):
            param_dict[key] = param.string_param
        elif param.HasField("double_param"):
            param_dict[key] = param.double_param
        elif param.HasField("uint64_param"):
            param_dict[key] = param.uint64_param
    return param_dict


def _validate_response_parameters(
    response_msg,
    expected_input_params: dict | None = None,
) -> dict:
    """Validate response parameters and optionally check for echoed input params.

    Args:
        response_msg: ModelInferResponse message
        expected_input_params: If provided, verify these parameters are echoed in response

    Returns:
        Dictionary of extracted parameter values
    """
    assert len(response_msg.parameters) > 0, "Expected parameters in response"

    param_dict = _extract_parameters(response_msg.parameters)
    logger.info(f"Response parameters: {param_dict}")

    # Always verify worker-added parameters
    assert "processed" in param_dict, "Expected 'processed' parameter added by worker"
    assert param_dict["processed"] is True, "Expected processed to be True"
    assert (
        "worker_name" in param_dict
    ), "Expected 'worker_name' parameter added by worker"

    # If input params provided, verify they're echoed with correct values and types
    if expected_input_params:
        for key, expected_value in expected_input_params.items():
            assert key in param_dict, f"Expected '{key}' parameter to be echoed"

            if isinstance(expected_value, float):
                assert (
                    abs(param_dict[key] - expected_value) < 0.001
                ), f"Expected {key}={expected_value}, got {param_dict[key]}"
            else:
                assert (
                    param_dict[key] == expected_value
                ), f"Expected {key}={expected_value}, got {param_dict[key]}"

            logger.info(f"✓ Parameter '{key}' echoed correctly: {param_dict[key]}")

    return param_dict


class DynamoGrpcFrontendProcess(ManagedProcess):
    """Process manager for Dynamo gRPC frontend."""

    def __init__(self, request, port: int = 8000):
        self.port = port
        command = [
            "python",
            "-m",
            "dynamo.frontend",
            "--kserve-grpc-server",
            "--http-port",
            str(port),
        ]

        log_dir = f"{request.node.name}_grpc_frontend"

        try:
            shutil.rmtree(log_dir)
        except FileNotFoundError:
            pass

        super().__init__(
            command=command,
            display_output=True,
            terminate_existing=True,
            log_dir=log_dir,
        )


class TensorParameterWorkerProcess(ManagedProcess):
    """Process manager for tensor parameter test worker."""

    def __init__(self, request):
        command = ["python", "tensor_params_worker.py"]

        log_dir = f"{request.node.name}_tensor_worker"

        try:
            shutil.rmtree(log_dir)
        except FileNotFoundError:
            pass

        super().__init__(
            command=command,
            display_output=True,
            terminate_existing=True,
            log_dir=log_dir,
        )


@pytest.fixture(scope="function")
def grpc_frontend(request, etcd_server: EtcdServer, nats_server: NatsServer):
    """Start Dynamo gRPC frontend."""
    frontend = DynamoGrpcFrontendProcess(request, port=8000)
    frontend.start()

    # Wait for frontend to be ready
    time.sleep(3)

    yield frontend

    frontend.stop()


@pytest.fixture(scope="function")
def tensor_worker(request, grpc_frontend):
    """Start tensor parameter worker."""
    worker = TensorParameterWorkerProcess(request)
    worker.start()

    # Wait for worker to register
    time.sleep(2)

    yield worker

    worker.stop()


@pytest.mark.e2e
@pytest.mark.pre_merge
@pytest.mark.gpu_0
@pytest.mark.parametrize(
    "request_params,test_description",
    [
        (
            {
                "batch_size": 8,
                "precision": "fp16",
                "enable_cache": True,
                "temperature": 0.9,
            },
            "with_parameters",
        ),
        (
            None,
            "backward_compatible",
        ),
    ],
    ids=["with_parameters", "backward_compatible"],
)
def test_grpc_tensor_request_level_parameters(
    grpc_frontend: DynamoGrpcFrontendProcess,
    tensor_worker: TensorParameterWorkerProcess,
    request_params: dict | None,
    test_description: str,
):
    """Test request-level parameters flow through gRPC frontend and backward compatibility.

    This test covers two scenarios:
    1. With parameters: Verifies all parameter types are echoed correctly
    2. Backward compatible: Verifies requests without parameters still work
    """
    if grpcclient is None:
        pytest.skip("tritonclient[grpc] not installed")

    client = grpcclient.InferenceServerClient(url=f"localhost:{grpc_frontend.port}")

    # Create input tensor
    input_data = np.array([[1.0, 2.0, 3.0, 4.0]], dtype=np.float32)
    inputs = [grpcclient.InferInput("input_data", input_data.shape, "FP32")]
    inputs[0].set_data_from_numpy(input_data)
    outputs = [grpcclient.InferRequestedOutput("input_data_out")]

    logger.info(f"Test scenario: {test_description}")
    logger.info(f"Sending request with parameters: {request_params}")

    # Send inference request (with or without parameters)
    response = client.infer(
        model_name="tensor-params-test",
        inputs=inputs,
        outputs=outputs,
        request_id=f"integration-test-{test_description}",
        parameters=request_params,
    )

    # Validate response using helper function
    response_msg = response.get_response()
    _validate_response_parameters(response_msg, expected_input_params=request_params)

    if request_params:
        logger.info("✅ All request-level parameters verified and echoed correctly")
    else:
        logger.info(
            "✅ Backward compatibility verified - request without params succeeded"
        )


@pytest.mark.e2e
@pytest.mark.pre_merge
@pytest.mark.gpu_0
def test_grpc_tensor_output_parameters(
    grpc_frontend: DynamoGrpcFrontendProcess,
    tensor_worker: TensorParameterWorkerProcess,
):
    """Test that output tensor-level parameters are returned from worker."""

    if grpcclient is None:
        pytest.skip("tritonclient[grpc] not installed")

    client = grpcclient.InferenceServerClient(url=f"localhost:{grpc_frontend.port}")

    # Create input tensor
    input_data = np.array([[5.0, 6.0]], dtype=np.float32)
    inputs = [grpcclient.InferInput("input_data", input_data.shape, "FP32")]
    inputs[0].set_data_from_numpy(input_data)
    outputs = [grpcclient.InferRequestedOutput("input_data_out")]

    logger.info("Sending request to test output tensor parameters")

    # Send inference request (no request params)
    response = client.infer(
        model_name="tensor-params-test",
        inputs=inputs,
        outputs=outputs,
        request_id="integration-test-002",
    )

    # Verify output tensor has parameters
    response_msg = response.get_response()
    assert len(response_msg.outputs) == 1, "Expected one output tensor"

    output_tensor = response_msg.outputs[0]
    assert len(output_tensor.parameters) > 0, "Expected parameters in output tensor"

    # Extract and verify parameters using helper
    tensor_params = _extract_parameters(output_tensor.parameters)
    logger.info(f"Output tensor parameters: {tensor_params}")

    # Verify worker added "echoed" parameter
    assert "echoed" in tensor_params, "Expected 'echoed' parameter in output tensor"
    assert tensor_params["echoed"] is True, "Expected echoed to be True"

    logger.info("✅ Output tensor parameters verified")

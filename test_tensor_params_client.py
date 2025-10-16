# SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

"""
Test Client for Tensor Parameters using Triton Client Library

Prerequisites:
    pip install tritonclient[grpc]

Usage:
    python test_tensor_params_client.py
"""

import numpy as np
import tritonclient.grpc as grpcclient


def test_request_level_params():
    """Test: Request-level parameters."""

    print("\n" + "=" * 60)
    print("TEST: Request-Level Parameters")
    print("=" * 60)

    # Create client
    client = grpcclient.InferenceServerClient(url="localhost:8000")

    # Create input tensor
    input_data = np.array([[1.0, 2.0, 3.0, 4.0]], dtype=np.float32)
    inputs = [grpcclient.InferInput("input_data", input_data.shape, "FP32")]
    inputs[0].set_data_from_numpy(input_data)

    # Create outputs
    outputs = [grpcclient.InferRequestedOutput("input_data_out")]

    # Set request-level parameters
    # Note: tritonclient automatically handles parameter type tagging
    params = {
        "batch_size": 32,
        "precision": "fp16",
        "enable_cache": True,
        "temperature": 0.8,
    }

    print("\n📤 Sending request with parameters:")
    for key, value in params.items():
        print(f"  {key}: {value} ({type(value).__name__})")

    # Send inference request
    response = client.infer(
        model_name="tensor-params-test",
        inputs=inputs,
        outputs=outputs,
        request_id="test-001",
        parameters=params,
    )

    # Display response parameters
    print("\n📥 Response parameters:")
    response_params = response.get_response().parameters

    if not response_params:
        print("  ⚠️  No parameters in response!")
        return False

    for key, param in response_params.items():
        # Extract value from InferParameter oneof
        if param.HasField("bool_param"):
            value = param.bool_param
            param_type = "bool"
        elif param.HasField("int64_param"):
            value = param.int64_param
            param_type = "int64"
        elif param.HasField("string_param"):
            value = param.string_param
            param_type = "string"
        elif param.HasField("double_param"):
            value = param.double_param
            param_type = "double"
        elif param.HasField("uint64_param"):
            value = param.uint64_param
            param_type = "uint64"
        else:
            value = "unknown"
            param_type = "unknown"
        print(f"  {key}: {value} ({param_type})")

    # Verify we got our input parameters back
    success = True
    if "processed" not in response_params:
        print("\n  ❌ Missing 'processed' parameter added by worker!")
        success = False
    else:
        print("\n  ✅ Found 'processed' parameter added by worker")

    # Check echoed parameters
    expected_echoes = ["batch_size", "precision", "enable_cache"]
    for param in expected_echoes:
        if param not in response_params:
            print(f"  ❌ Missing echoed parameter: {param}")
            success = False

    if success:
        print("\n✅ Test passed!")
    else:
        print("\n❌ Test failed!")

    return success


def test_output_tensor_params():
    """Test: Output tensor-level parameters (added by worker)."""

    print("\n" + "=" * 60)
    print("TEST: Output Tensor Parameters")
    print("=" * 60)

    # Create client
    client = grpcclient.InferenceServerClient(url="localhost:8000")

    # Create input tensor (no input parameters, but worker will add output params)
    input_data = np.array([[5.0, 6.0]], dtype=np.float32)
    inputs = [grpcclient.InferInput("input_data", input_data.shape, "FP32")]
    inputs[0].set_data_from_numpy(input_data)

    # Create outputs
    outputs = [grpcclient.InferRequestedOutput("input_data_out")]

    print("\n📤 Sending request (no input tensor params)")

    # Send inference request
    response = client.infer(
        model_name="tensor-params-test",
        inputs=inputs,
        outputs=outputs,
        request_id="test-002",
    )

    # Display output tensor parameters
    print("\n📥 Output tensor parameters:")
    success = True

    for output in response.get_response().outputs:
        print(f"\n  Tensor: {output.name}")
        if not output.parameters:
            print("    ⚠️  No parameters!")
            success = False
            continue

        for key, param in output.parameters.items():
            if param.HasField("bool_param"):
                value = param.bool_param
                param_type = "bool"
            elif param.HasField("int64_param"):
                value = param.int64_param
                param_type = "int64"
            elif param.HasField("string_param"):
                value = param.string_param
                param_type = "string"
            elif param.HasField("double_param"):
                value = param.double_param
                param_type = "double"
            else:
                value = "unknown"
                param_type = "unknown"
            print(f"    {key}: {value} ({param_type})")

        # Check for "echoed" parameter
        if "echoed" in output.parameters:
            print("    ✅ Found 'echoed' parameter added by worker")
        else:
            print("    ❌ Missing 'echoed' parameter!")
            success = False

    if success:
        print("\n✅ Test passed!")
    else:
        print("\n❌ Test failed!")

    return success


def main():
    """Run all tests."""

    print("\n" + "=" * 60)
    print("🧪 Tensor Parameters Test Suite")
    print("=" * 60)
    print("\nPrerequisites:")
    print(
        "  1. Frontend: python -m dynamo.frontend --kserve-grpc-server --http-port 8000"
    )
    print("  2. Worker: python tensor_params_worker.py")

    try:
        # Test request-level parameters
        test1_passed = test_request_level_params()

        # Test output tensor parameters
        test2_passed = test_output_tensor_params()

        # Summary
        print("\n" + "=" * 60)
        if test1_passed and test2_passed:
            print("✅ ALL TESTS PASSED!")
            print("\nVerified:")
            print("  ✓ Request-level parameters flow through")
            print("  ✓ Worker can add parameters")
            print("  ✓ Output tensor parameters work")
        else:
            print("❌ SOME TESTS FAILED")
            print(f"  Request params: {'✅' if test1_passed else '❌'}")
            print(f"  Tensor params: {'✅' if test2_passed else '❌'}")
        print("=" * 60 + "\n")

    except Exception as e:
        print(f"\n❌ Test failed with exception: {e}")
        print("\nTroubleshooting:")
        print("  - Is the frontend running on port 8000?")
        print("  - Is the worker running and registered?")
        print("  - Are etcd and NATS running?")
        import traceback

        traceback.print_exc()


if __name__ == "__main__":
    main()

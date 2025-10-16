# SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

"""Mock Tensor Worker for Testing gRPC Parameters."""

import uvloop

from dynamo.llm import ModelInput, ModelRuntimeConfig, ModelType, register_llm
from dynamo.runtime import DistributedRuntime, dynamo_worker


async def generate(request, context):
    """Echo tensors and add parameters to demonstrate they work."""

    print(f"\n{'='*50}")
    print("📥 Request received")
    print(f"Request params: {request.get('parameters', {})}")

    for tensor in request.get("tensors", []):
        name = tensor["metadata"]["name"]
        params = tensor["metadata"].get("parameters", {})
        print(f"Tensor '{name}' params: {params}")

    # Echo input tensors with added parameters
    output_tensors = []
    for tensor in request.get("tensors", []):
        metadata = tensor["metadata"]

        # Echo parameters and add new one in tagged format
        output_params = metadata.get("parameters", {}).copy()
        output_params["echoed"] = {"bool": True}  # Tagged format

        output_tensors.append(
            {
                "metadata": {
                    "name": metadata["name"] + "_out",
                    "data_type": metadata["data_type"],
                    "shape": metadata["shape"],
                    "parameters": output_params if output_params else None,
                },
                "data": tensor["data"],
            }
        )

    # Echo request params and add new ones in tagged format
    # IMPORTANT: Each parameter must be a SINGLE tagged value
    response_params = request.get("parameters", {}).copy()
    response_params["processed"] = {"bool": True}  # Valid: single tag
    response_params["worker_name"] = {
        "string": "tensor_params_worker"
    }  # Valid: single tag
    response_params["request_count"] = {"int64": 1}  # Valid: single tag

    print(f"📤 Response params: {response_params}")
    print(f"{'='*50}\n")

    yield {
        "model": request.get("model"),
        "tensors": output_tensors,
        "parameters": response_params if response_params else None,
    }


@dynamo_worker(static=False)
async def main(runtime: DistributedRuntime):
    """Register and serve tensor model."""

    print("🚀 Tensor Parameters Worker")

    component = runtime.namespace("test").component("tensor")
    await component.create_service()
    endpoint = component.endpoint("generate")

    # Register tensor model
    runtime_config = ModelRuntimeConfig()
    runtime_config.set_tensor_model_config(
        {
            "name": "tensor-params-test",
            "inputs": [{"name": "input_data", "data_type": "Float32", "shape": [-1]}],
            "outputs": [
                {"name": "input_data_out", "data_type": "Float32", "shape": [-1]}
            ],
        }
    )

    await register_llm(
        ModelInput.Tensor,
        ModelType.TensorBased,
        endpoint,
        "Qwen/Qwen3-0.6B",
        model_name="tensor-params-test",
        runtime_config=runtime_config,
    )

    print("✅ Registered: tensor-params-test")
    print("⏳ Ready\n")

    await endpoint.serve_endpoint(generate)


if __name__ == "__main__":
    uvloop.run(main())

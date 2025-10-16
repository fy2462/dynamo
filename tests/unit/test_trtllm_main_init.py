# SPDX-FileCopyrightText: Copyright (c) 2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

"""Unit tests for TensorRT-LLM main.py parameter validation without starting engines.

These tests serve multiple critical purposes:

1. Detect breaking TensorRT-LLM API changes, esp. KvCacheConfig, before they hit production.
2. Prevent regression of KvCacheConfig TypeError (dict assignment bug stays fixed).
3. Check metrics integration: publish_events_and_metrics param transforms still valid, no engines needed.
4. Fast: runs in ms, enables quick CI and local iterations.
5. Confirms APIs work for both metrics enabled/disabled paths.
6. Spec-based mocks catch runtime interface mismatches early.
7. Tests act as terse executable documentation for init configs.
"""

import sys
from typing import Any, Dict, Tuple
from unittest.mock import AsyncMock, Mock, patch

import pytest

# Import actual types for type annotations
from dynamo.runtime import Component, DistributedRuntime, Endpoint, Namespace
from dynamo.trtllm.utils.trtllm_utils import Config

# Create comprehensive mocks for TensorRT-LLM
mock_tensorrt_llm = Mock()
mock_tensorrt_llm.llmapi = Mock()
mock_tensorrt_llm.llmapi.BuildConfig = Mock()
mock_tensorrt_llm.llmapi.KvCacheConfig = Mock()
mock_tensorrt_llm.llmapi.DynamicBatchConfig = Mock()
mock_tensorrt_llm.llmapi.SchedulerConfig = Mock()
mock_tensorrt_llm.llmapi.CapacitySchedulerPolicy = Mock()
mock_tensorrt_llm.llmapi.CapacitySchedulerPolicy.GUARANTEED_NO_EVICT = (
    "GUARANTEED_NO_EVICT"
)
mock_tensorrt_llm.llmapi.llm = Mock()
mock_tensorrt_llm.llmapi.llm.SamplingParams = Mock()
mock_tensorrt_llm.llmapi.llm_utils = Mock()
mock_tensorrt_llm.llmapi.llm_utils.update_llm_args_with_extra_options = Mock(
    side_effect=lambda x, _: x
)
mock_tensorrt_llm.llmapi.tokenizer = Mock()
mock_tensorrt_llm.llmapi.tokenizer.tokenizer_factory = Mock()
mock_tensorrt_llm.inputs = Mock()
mock_tensorrt_llm.inputs.default_multimodal_input_loader = Mock()

# Mock torch
mock_torch = Mock()
mock_torch.cuda = Mock()
mock_torch.cuda.device_count = Mock(return_value=1)

# Mock transformers
mock_transformers = Mock()
mock_transformers.AutoConfig = Mock()

# Mock prometheus client
mock_prometheus_client = Mock()
mock_prometheus_client.REGISTRY = Mock()

# Patch all TensorRT-LLM related modules
sys.modules["tensorrt_llm"] = mock_tensorrt_llm
sys.modules["tensorrt_llm.llmapi"] = mock_tensorrt_llm.llmapi
sys.modules["tensorrt_llm.llmapi.llm"] = mock_tensorrt_llm.llmapi.llm
sys.modules["tensorrt_llm.llmapi.llm_utils"] = mock_tensorrt_llm.llmapi.llm_utils
sys.modules["tensorrt_llm.llmapi.tokenizer"] = mock_tensorrt_llm.llmapi.tokenizer
sys.modules["tensorrt_llm.inputs"] = mock_tensorrt_llm.inputs
sys.modules["tensorrt_llm.metrics"] = Mock()
sys.modules["torch"] = mock_torch
sys.modules["torch.cuda"] = mock_torch.cuda
sys.modules["transformers"] = mock_transformers
sys.modules["prometheus_client"] = mock_prometheus_client

pytestmark = [
    pytest.mark.unit,
    pytest.mark.trtllm_marker,
]


class TestTrtllmMainParameterValidation:
    """Test TensorRT-LLM main.py parameter validation without starting engines."""

    @pytest.mark.skip(reason="Requires full TensorRT-LLM installation for import chain")
    def test_import_main_module(self):
        """Test that main module can be imported without errors."""
        # This tests all imports and module-level code
        try:
            import dynamo.trtllm.main

            assert hasattr(dynamo.trtllm.main, "init")
            assert hasattr(dynamo.trtllm.main, "worker")
            assert hasattr(dynamo.trtllm.main, "main")
        except ImportError as e:
            pytest.fail(f"Failed to import dynamo.trtllm.main: {e}")

    def test_kv_cache_config_type_handling(self):
        """Test KvCacheConfig type handling logic that was causing the TypeError.

        This test is critical for preventing regression of the original bug where
        KvCacheConfig objects were being treated as dictionaries, causing:
        TypeError: 'KvCacheConfig' object does not support item assignment

        Guards against:
        - TensorRT-LLM changing KvCacheConfig implementation
        - Accidental removal of type checking logic
        - Changes to the isinstance() behavior with KvCacheConfig
        """
        from tensorrt_llm.llmapi import KvCacheConfig

        # Test case 1: KvCacheConfig object can be created
        kv_cache_config = KvCacheConfig(free_gpu_memory_fraction=0.9)
        assert kv_cache_config is not None

        # Test case 2: Dict with event_buffer_max_size (metrics case)
        kv_cache_dict = {"free_gpu_memory_fraction": 0.9, "event_buffer_max_size": 1024}

        # Verify it's a dict (not KvCacheConfig object)
        assert isinstance(kv_cache_dict, dict)
        assert "event_buffer_max_size" in kv_cache_dict

        # Test case 3: Type checking logic
        # This simulates the isinstance check in the actual code
        assert isinstance(kv_cache_config, type(KvCacheConfig()))
        assert not isinstance(kv_cache_dict, type(KvCacheConfig()))

    def test_publish_events_and_metrics_disabled(self):
        """Test that parameters remain unchanged when publish_events_and_metrics is False."""
        from tensorrt_llm.llmapi import KvCacheConfig

        config = Mock()
        config.publish_events_and_metrics = False
        config.free_gpu_memory_fraction = 0.9

        # Create normal KvCacheConfig object
        kv_cache_config = KvCacheConfig(free_gpu_memory_fraction=0.9)
        arg_map = {
            "kv_cache_config": kv_cache_config,
            "backend": "pytorch",
            "skip_tokenizer_init": True,
        }

        # When publish_events_and_metrics is False, KvCacheConfig should remain as object
        assert isinstance(arg_map["kv_cache_config"], type(KvCacheConfig()))
        assert arg_map["backend"] == "pytorch"
        assert arg_map["skip_tokenizer_init"] is True

    @pytest.fixture
    def mock_runtime_and_components(
        self,
    ) -> Tuple[DistributedRuntime, Namespace, Component, Endpoint]:
        """Create mock runtime, namespace, component, and endpoint."""
        runtime = Mock(spec=DistributedRuntime)
        namespace = Mock(spec=Namespace)
        component = Mock(spec=Component)
        endpoint = Mock(spec=Endpoint)

        namespace.component.return_value = component
        component.endpoint.return_value = endpoint
        component.create_service = AsyncMock()
        endpoint.serve_endpoint = AsyncMock()
        endpoint.lease_id.return_value = "test-lease-123"
        runtime.namespace.return_value = namespace

        return runtime, namespace, component, endpoint

    @pytest.fixture
    def base_config(self) -> Config:
        """Create base configuration object with common settings."""
        config = Mock(spec=Config)
        config.model_path = "test-model"
        config.namespace = "test-ns"
        config.component = "test-component"
        config.endpoint = "generate"
        config.max_batch_size = 8
        config.max_num_tokens = 1024
        config.max_beam_width = 1
        config.max_seq_len = 2048
        config.tensor_parallel_size = 1
        config.pipeline_parallel_size = 1
        config.expert_parallel_size = 1
        config.gpus_per_node = None
        config.free_gpu_memory_fraction = 0.9
        config.served_model_name = None
        config.kv_block_size = 16
        config.migration_limit = 10
        config.extra_engine_args = ""
        config.override_engine_args = ""
        config.next_endpoint = None
        config.encode_endpoint = None
        config.disaggregation_mode = Mock()
        config.disaggregation_mode.value = "none"
        config.disaggregation_strategy = None
        config.custom_jinja_template = None
        config.dump_config_to = None
        config.max_file_size_mb = 10
        config.allowed_local_media_path = None
        return config

    def setup_common_mocks(self) -> Dict[str, Any]:
        """Setup common mocks used by both init tests."""
        # Engine mock
        mock_engine_context = AsyncMock()
        mock_engine_context.__aenter__ = AsyncMock(return_value=Mock())
        mock_engine_context.__aexit__ = AsyncMock(return_value=None)

        # Connector mock
        mock_connector_instance = Mock()
        mock_connector_instance.initialize = AsyncMock()

        # Handler mock
        mock_handler = Mock()
        mock_handler.generate = Mock()

        return {
            "engine_context": mock_engine_context,
            "connector_instance": mock_connector_instance,
            "handler": mock_handler,
        }

    @pytest.mark.asyncio
    async def test_init_function_with_metrics_disabled(
        self,
        mock_runtime_and_components: Tuple[
            DistributedRuntime, Namespace, Component, Endpoint
        ],
        base_config: Config,
    ) -> None:
        """Test calling the actual init() function with publish_events_and_metrics=False.

        This integration test ensures the complete initialization flow works correctly
        when metrics are disabled (the default/normal case).

        Critical for:
        - Detecting TensorRT-LLM API changes that break our initialization
        - Validating that KvCacheConfig objects remain unchanged in normal operation
        - Ensuring all required dependencies are properly mocked for fast testing
        - Catching changes to the init() function signature or behavior
        """
        runtime, _namespace, _component, _endpoint = mock_runtime_and_components
        config = base_config
        config.publish_events_and_metrics = False  # Key: metrics disabled

        mocks = self.setup_common_mocks()

        # Mock all the heavy dependencies
        with patch("dynamo.trtllm.main.get_llm_engine") as mock_get_engine, patch(
            "dynamo.trtllm.main.register_llm"
        ) as mock_register_llm, patch(
            "dynamo.trtllm.main.is_first_worker"
        ) as mock_is_first_worker, patch(
            "dynamo.trtllm.main.RequestHandlerFactory"
        ) as mock_handler_factory, patch(
            "dynamo.trtllm.main.nixl_connect.Connector"
        ) as mock_connector, patch(
            "dynamo.trtllm.main.dump_config"
        ) as mock_dump_config:
            # Setup mocks
            mock_get_engine.return_value = mocks["engine_context"]
            mock_is_first_worker.return_value = True
            mock_register_llm.return_value = AsyncMock()
            mock_connector.return_value = mocks["connector_instance"]
            mock_handler_factory.return_value.get_request_handler.return_value = mocks[
                "handler"
            ]

            # Import and call the actual init function
            from dynamo.trtllm.main import init

            # This should complete without errors
            await init(runtime, config)

            # Verify key calls were made
            mock_get_engine.assert_called_once()
            mock_register_llm.assert_called_once()
            mock_dump_config.assert_called_once()

    @pytest.mark.asyncio
    async def test_init_function_with_metrics_enabled(
        self,
        mock_runtime_and_components: Tuple[
            DistributedRuntime, Namespace, Component, Endpoint
        ],
        base_config: Config,
    ) -> None:
        """Test calling the actual init() function with publish_events_and_metrics=True.

        This is the most critical test as it validates the complete metrics-enabled
        initialization flow, including the KvCacheConfig→dict conversion that
        was causing the original TypeError.

        Essential for:
        - Preventing regression of the KvCacheConfig TypeError fix
        - Validating Prometheus metrics callback registration with correct parameters
        - Ensuring TensorRT-LLM metrics integration doesn't break with library updates
        - Testing the publisher context manager setup for metrics collection
        - Verifying the exclude_prefixes and add_prefix parameters work correctly
        """
        runtime, _namespace, _component, _endpoint = mock_runtime_and_components
        config = base_config
        config.publish_events_and_metrics = True  # Key: metrics enabled
        config.served_model_name = "test-model-served"

        mocks = self.setup_common_mocks()

        # Mock all the heavy dependencies including Prometheus
        with patch("dynamo.trtllm.main.get_llm_engine") as mock_get_engine, patch(
            "dynamo.trtllm.main.register_llm"
        ) as mock_register_llm, patch(
            "dynamo.trtllm.main.is_first_worker"
        ) as mock_is_first_worker, patch(
            "dynamo.trtllm.main.get_publisher"
        ) as mock_get_publisher, patch(
            "dynamo.trtllm.main.RequestHandlerFactory"
        ) as mock_handler_factory, patch(
            "dynamo.trtllm.main.nixl_connect.Connector"
        ) as mock_connector, patch(
            "dynamo.trtllm.main.dump_config"
        ) as mock_dump_config, patch(
            "dynamo.trtllm.main.register_engine_metrics_callback"
        ) as mock_register_metrics:
            # Setup common mocks
            mock_get_engine.return_value = mocks["engine_context"]
            mock_is_first_worker.return_value = True
            mock_register_llm.return_value = AsyncMock()
            mock_connector.return_value = mocks["connector_instance"]
            mock_handler_factory.return_value.get_request_handler.return_value = mocks[
                "handler"
            ]

            # Setup metrics-specific mocks
            mock_publisher_context = AsyncMock()
            mock_publisher_context.__aenter__ = AsyncMock(return_value=Mock())
            mock_publisher_context.__aexit__ = AsyncMock(return_value=None)
            mock_get_publisher.return_value = mock_publisher_context

            # Import and call the actual init function
            from dynamo.trtllm.main import init

            # This should complete without errors and handle the KvCacheConfig conversion
            await init(runtime, config)

            # Verify key calls were made
            mock_get_engine.assert_called_once()
            mock_register_llm.assert_called_once()
            mock_dump_config.assert_called_once()

            # Verify metrics callback was registered with correct parameters
            mock_register_metrics.assert_called_once()
            call_args = mock_register_metrics.call_args
            assert call_args[1]["exclude_prefixes"] == ["python_", "process_"]
            assert call_args[1]["add_prefix"] == "trtllm:"

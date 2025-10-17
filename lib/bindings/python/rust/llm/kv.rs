// SPDX-FileCopyrightText: Copyright (c) 2024-2025 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use pythonize::{depythonize, pythonize};
use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use tokio_stream::StreamExt;

use super::*;
use crate::Component;
use llm_rs::kv_router::indexer::KvIndexerInterface;
use llm_rs::kv_router::indexer::compute_block_hash_for_seq;
use llm_rs::kv_router::protocols::ForwardPassMetrics as RsForwardPassMetrics;
use llm_rs::kv_router::protocols::KvStats as RsKvStats;
use llm_rs::kv_router::protocols::SpecDecodeStats as RsSpecDecodeStats;
use llm_rs::kv_router::protocols::WorkerStats as RsWorkerStats;
use rs::pipeline::{AsyncEngine, SingleIn};
use rs::traits::events::EventSubscriber;
use tracing;

use llm_rs::kv_router::protocols::*;
use llm_rs::kv_router::publisher::{KvEventSourceConfig, create_stored_blocks};
use llm_rs::protocols::common::{OutputOptions, SamplingOptions, StopConditions};

#[pyfunction]
pub fn compute_block_hash_for_seq_py(tokens: Vec<u32>, kv_block_size: usize) -> PyResult<Vec<u64>> {
    if kv_block_size == 0 {
        return Err(to_pyerr(anyhow::anyhow!("kv_block_size cannot be 0")));
    }

    let hashes = compute_block_hash_for_seq(&tokens, kv_block_size as u32);
    Ok(hashes.into_iter().map(|h| h.0).collect())
}

#[pyclass]
pub(crate) struct WorkerMetricsPublisher {
    inner: Arc<llm_rs::kv_router::publisher::WorkerMetricsPublisher>,
}

#[pymethods]
impl WorkerMetricsPublisher {
    #[new]
    fn new() -> PyResult<Self> {
        let inner =
            llm_rs::kv_router::publisher::WorkerMetricsPublisher::new().map_err(to_pyerr)?;
        Ok(Self {
            inner: inner.into(),
        })
    }

    #[pyo3(signature = (component, metrics_labels = None))]
    #[allow(unused_variables)]
    fn create_endpoint<'p>(
        &self,
        py: Python<'p>,
        component: Component,
        metrics_labels: Option<Vec<(String, String)>>, // TODO: fully remove this
    ) -> PyResult<Bound<'p, PyAny>> {
        // Emit deprecation warning if metrics_labels is provided
        if metrics_labels.is_some() {
            let warnings = py.import("warnings")?;
            warnings.call_method1(
                "warn",
                (
                    "The 'metrics_labels' parameter is deprecated and no longer used. It will be removed in a future version.",
                    py.get_type::<pyo3::exceptions::PyDeprecationWarning>(),
                ),
            )?;
        }

        let rs_publisher = self.inner.clone();
        let rs_component = component.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Register Prometheus metrics first
            rs_publisher
                .register_prometheus_metrics(&rs_component)
                .map_err(to_pyerr)?;

            rs_publisher
                .create_endpoint(rs_component)
                .await
                .map_err(to_pyerr)?;
            Ok(())
        })
    }

    #[pyo3(signature = (metrics))]
    fn publish(&self, _py: Python, metrics: &ForwardPassMetrics) -> PyResult<()> {
        // Create and publish the complete metrics
        self.inner
            .publish(metrics.0.clone().into())
            .map_err(to_pyerr)
    }
}

#[pyclass]
#[derive(Clone)]
pub struct ZmqKvEventPublisherConfig {
    #[pyo3(get, set)]
    pub worker_id: WorkerId,
    #[pyo3(get, set)]
    pub kv_block_size: usize,
    #[pyo3(get, set)]
    pub zmq_endpoint: String,
    #[pyo3(get, set)]
    pub zmq_topic: String,
}

#[pymethods]
impl ZmqKvEventPublisherConfig {
    #[new]
    #[pyo3(signature = (
        worker_id,
        kv_block_size,
        zmq_endpoint = "tcp://127.0.0.1:5557".to_string(),
        zmq_topic = "".to_string()
    ))]
    pub fn new(
        worker_id: WorkerId,
        kv_block_size: usize,
        zmq_endpoint: String,
        zmq_topic: String,
    ) -> Self {
        Self {
            worker_id,
            kv_block_size,
            zmq_endpoint,
            zmq_topic,
        }
    }
}

#[pyclass]
pub(crate) struct ZmqKvEventPublisher {
    inner: llm_rs::kv_router::publisher::KvEventPublisher,
}

#[pymethods]
impl ZmqKvEventPublisher {
    #[new]
    fn new(component: Component, config: ZmqKvEventPublisherConfig) -> PyResult<Self> {
        let inner = llm_rs::kv_router::publisher::KvEventPublisher::new(
            component.inner,
            config.worker_id,
            config.kv_block_size as u32,
            Some(KvEventSourceConfig::Zmq {
                endpoint: config.zmq_endpoint,
                topic: config.zmq_topic,
            }),
        )
        .map_err(to_pyerr)?;
        Ok(Self { inner })
    }

    fn shutdown(&mut self) {
        self.inner.shutdown()
    }
}

/// A ZMQ-based key-value cache event listener that operates independently
/// of the dynamo runtime or event plane infrastructure.
#[pyclass]
pub(crate) struct ZmqKvEventListener {
    event_receiver: Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<KvCacheEvent>>>,
    shutdown_token: tokio_util::sync::CancellationToken,
}

#[pymethods]
impl ZmqKvEventListener {
    #[new]
    fn new(zmq_endpoint: String, zmq_topic: String, kv_block_size: usize) -> PyResult<Self> {
        if kv_block_size == 0 {
            return Err(to_pyerr(anyhow::anyhow!("kv_block_size cannot be 0")));
        }

        let runtime = pyo3_async_runtimes::tokio::get_runtime();
        runtime.block_on(async {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<KvCacheEvent>();
            let shutdown_token = tokio_util::sync::CancellationToken::new();

            tokio::spawn(llm_rs::kv_router::publisher::start_zmq_listener(
                zmq_endpoint,
                zmq_topic,
                tx,
                shutdown_token.clone(),
                kv_block_size as u32,
            ));

            Ok(Self {
                event_receiver: Arc::new(tokio::sync::Mutex::new(rx)),
                shutdown_token,
            })
        })
    }

    fn get_events<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        let receiver = self.event_receiver.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut rx = receiver.lock().await;
            let mut events = Vec::new();

            // Drain all available events
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }

            // Convert events to JSON strings
            let json_events: Result<Vec<String>, _> =
                events.iter().map(serde_json::to_string).collect();

            match json_events {
                Ok(json_strings) => Ok(json_strings),
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Failed to serialize events to JSON: {}",
                    e
                ))),
            }
        })
    }
}

// manual shutdown needed as it's not tied to the dynamo DRT
impl Drop for ZmqKvEventListener {
    fn drop(&mut self) {
        self.shutdown_token.cancel();
    }
}

#[pyclass]
pub(crate) struct KvEventPublisher {
    inner: Arc<llm_rs::kv_router::publisher::KvEventPublisher>,
    kv_block_size: usize,
    dp_rank: DpRank,
    warning_count: Arc<AtomicU32>,
}

#[pymethods]
impl KvEventPublisher {
    #[new]
    #[pyo3(signature = (component, worker_id, kv_block_size, dp_rank=0))]
    fn new(
        component: Component,
        worker_id: WorkerId,
        kv_block_size: usize,
        dp_rank: DpRank,
    ) -> PyResult<Self> {
        if kv_block_size == 0 {
            return Err(to_pyerr(anyhow::anyhow!("kv_block_size cannot be 0")));
        }

        let inner = llm_rs::kv_router::publisher::KvEventPublisher::new(
            component.inner,
            worker_id,
            kv_block_size as u32,
            None,
        )
        .map_err(to_pyerr)?;

        Ok(Self {
            inner: inner.into(),
            kv_block_size,
            dp_rank,
            warning_count: Arc::new(AtomicU32::new(0)),
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (event_id, token_ids, num_block_tokens, block_hashes, lora_id, parent_hash=None))]
    fn publish_stored(
        &mut self,
        _py: Python,
        event_id: u64,
        token_ids: Vec<u32>,
        num_block_tokens: Vec<u64>,
        block_hashes: Vec<i64>,
        lora_id: u64,
        parent_hash: Option<i64>,
    ) -> PyResult<()> {
        let block_hashes_u64: Vec<u64> = block_hashes.iter().map(|&h| h as u64).collect();
        let event = KvCacheEvent {
            event_id,
            data: KvCacheEventData::Stored(KvCacheStoreData {
                parent_hash: parent_hash.map(ExternalSequenceBlockHash::from),
                blocks: create_stored_blocks(
                    self.kv_block_size as u32,
                    &token_ids,
                    &num_block_tokens,
                    &block_hashes_u64,
                    lora_id,
                    &self.warning_count,
                ),
            }),
            dp_rank: self.dp_rank,
        };

        self.inner.publish(event).map_err(to_pyerr)
    }

    fn publish_removed(&self, _py: Python, event_id: u64, block_hashes: Vec<i64>) -> PyResult<()> {
        let block_hashes: Vec<ExternalSequenceBlockHash> = block_hashes
            .into_iter()
            .map(ExternalSequenceBlockHash::from)
            .collect();
        let event = KvCacheEvent {
            event_id,
            data: KvCacheEventData::Removed(KvCacheRemoveData { block_hashes }),
            dp_rank: self.dp_rank,
        };

        self.inner.publish(event).map_err(to_pyerr)
    }
}

#[pyclass]
#[derive(Clone)]
pub(crate) struct OverlapScores {
    inner: llm_rs::kv_router::indexer::OverlapScores,
}

#[pymethods]
impl OverlapScores {
    #[getter]
    fn scores(&self) -> HashMap<(i64, u32), u32> {
        // Return scores with full WorkerWithDpRank granularity as (worker_id, dp_rank) tuples
        self.inner
            .scores
            .iter()
            .map(|(worker, score)| ((worker.worker_id, worker.dp_rank), *score))
            .collect()
    }

    #[getter]
    fn frequencies(&self) -> Vec<usize> {
        self.inner.frequencies.clone()
    }
}

// NOTE: the user needs to guarantee that this stays single threaded in Python land
#[pyclass(unsendable)]
pub(crate) struct RadixTree {
    inner: llm_rs::kv_router::indexer::RadixTree,
}

#[pymethods]
impl RadixTree {
    #[new]
    #[pyo3(signature = (expiration_duration_secs=None))]
    fn new(expiration_duration_secs: Option<f64>) -> PyResult<Self> {
        let expiration_duration = expiration_duration_secs.map(std::time::Duration::from_secs_f64);
        let inner = llm_rs::kv_router::indexer::RadixTree::new_with_frequency(expiration_duration);
        Ok(Self { inner })
    }

    #[pyo3(signature = (sequence, early_exit=false))]
    fn find_matches(
        &self,
        _py: Python,
        sequence: Vec<u64>,
        early_exit: bool,
    ) -> PyResult<OverlapScores> {
        let local_block_hashes: Vec<llm_rs::kv_router::protocols::LocalBlockHash> = sequence
            .into_iter()
            .map(llm_rs::kv_router::protocols::LocalBlockHash)
            .collect();

        let rs_overlap_scores = self.inner.find_matches(local_block_hashes, early_exit);
        Ok(OverlapScores {
            inner: rs_overlap_scores,
        })
    }

    fn apply_event(
        &mut self,
        _py: Python,
        worker_id: WorkerId,
        kv_cache_event_bytes: &[u8],
    ) -> PyResult<()> {
        let kv_cache_event: llm_rs::kv_router::protocols::KvCacheEvent =
            serde_json::from_slice(kv_cache_event_bytes).map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Failed to deserialize KvCacheEvent: {}",
                    e
                ))
            })?;

        let router_event = llm_rs::kv_router::indexer::RouterEvent::new(worker_id, kv_cache_event);
        let _ = self.inner.apply_event(router_event);
        Ok(())
    }

    fn remove_worker(&mut self, _py: Python, worker_id: WorkerId) -> PyResult<()> {
        self.inner.remove_worker(worker_id);
        Ok(())
    }

    fn clear_all_blocks(&mut self, _py: Python, worker_id: WorkerId) -> PyResult<()> {
        self.inner.clear_all_blocks(worker_id);
        Ok(())
    }
}

#[pyclass]
pub(crate) struct KvIndexer {
    inner: Arc<llm_rs::kv_router::indexer::KvIndexer>,
}

#[pymethods]
impl KvIndexer {
    #[new]
    #[pyo3(signature = (component, kv_block_size, consumer_uuid=None))]
    fn new(
        component: Component,
        kv_block_size: usize,
        consumer_uuid: Option<String>,
    ) -> PyResult<Self> {
        let runtime = pyo3_async_runtimes::tokio::get_runtime();
        runtime.block_on(async {
            let cancellation_token = component.inner.drt().runtime().child_token();
            let kv_indexer_metrics =
                llm_rs::kv_router::indexer::KvIndexerMetrics::from_component(&component.inner);
            let inner: Arc<llm_rs::kv_router::indexer::KvIndexer> =
                llm_rs::kv_router::indexer::KvIndexer::new(
                    cancellation_token.clone(),
                    kv_block_size as u32,
                    kv_indexer_metrics,
                )
                .into();

            // Use the shared start_kv_router_background function for event consumption
            // Pass None for snapshot_tx and get_workers_tx to skip snapshot handling in Python bindings
            llm_rs::kv_router::subscriber::start_kv_router_background(
                component.inner.clone(),
                consumer_uuid.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                inner.event_sender(),
                inner.remove_worker_sender(),
                None,
                None,
                cancellation_token,
                None,
                true,
            )
            .await
            .map_err(to_pyerr)?;

            Ok(Self { inner })
        })
    }

    fn block_size(&self) -> usize {
        self.inner.block_size() as usize
    }

    fn find_matches<'p>(&self, py: Python<'p>, sequence: Vec<u64>) -> PyResult<Bound<'p, PyAny>> {
        let indexer = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let local_block_hashes: Vec<llm_rs::kv_router::protocols::LocalBlockHash> = sequence
                .into_iter()
                .map(llm_rs::kv_router::protocols::LocalBlockHash)
                .collect();

            let rs_overlap_scores = indexer
                .find_matches(local_block_hashes)
                .await
                .map_err(to_pyerr)?;
            Ok(OverlapScores {
                inner: rs_overlap_scores,
            })
        })
    }

    fn find_matches_for_request<'p>(
        &self,
        py: Python<'p>,
        token_ids: Vec<u32>,
        _lora_id: u64,
    ) -> PyResult<Bound<'p, PyAny>> {
        let indexer = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let rs_overlap_scores = indexer
                .find_matches_for_request(token_ids.as_slice())
                .await
                .map_err(to_pyerr)?;
            Ok(OverlapScores {
                inner: rs_overlap_scores,
            })
        })
    }
}

/// Bindings for the approximate KV indexer. We need to exactly match the regular KV Indexer
/// interface, so that the router can switch between the two.
#[pyclass]
pub(crate) struct ApproxKvIndexer {
    inner: Arc<llm_rs::kv_router::approx::ApproxKvIndexer>,
}

#[pymethods]
impl ApproxKvIndexer {
    #[new]
    fn new(component: Component, kv_block_size: usize, ttl_secs: f64) -> PyResult<Self> {
        let ttl = tokio::time::Duration::from_secs_f64(ttl_secs);
        let inner = Arc::new(llm_rs::kv_router::approx::ApproxKvIndexer::new(
            component.inner.drt().runtime().child_token(),
            kv_block_size as u32,
            ttl,
        ));
        Ok(Self { inner })
    }

    fn block_size(&self) -> u32 {
        self.inner.block_size()
    }

    fn find_matches_for_request<'p>(
        &self,
        py: Python<'p>,
        token_ids: Vec<u32>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let indexer = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let rs_overlap_scores = indexer
                .find_matches_for_request(token_ids.as_slice())
                .await
                .map_err(to_pyerr)?;
            Ok(OverlapScores {
                inner: rs_overlap_scores,
            })
        })
    }

    #[pyo3(signature = (tokens, worker_id, dp_rank=0))]
    fn process_routing_decision_for_request<'p>(
        &self,
        py: Python<'p>,
        tokens: Vec<u32>,
        worker_id: WorkerId,
        dp_rank: DpRank,
    ) -> PyResult<Bound<'p, PyAny>> {
        let indexer = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let worker = llm_rs::kv_router::protocols::WorkerWithDpRank::new(worker_id, dp_rank);
            indexer
                .process_routing_decision_for_request(tokens.as_slice(), worker)
                .await
                .map_err(to_pyerr)?;
            Ok(())
        })
    }
}

#[pyclass]
pub(crate) struct KvRecorder {
    inner: Arc<llm_rs::kv_router::recorder::KvRecorder>,
}

#[pymethods]
impl KvRecorder {
    #[new]
    #[pyo3(signature = (component, output_path=None, max_lines_per_file=None, max_count=None, max_time=None))]
    fn new(
        component: Component,
        output_path: Option<String>,
        max_lines_per_file: Option<usize>,
        max_count: Option<usize>,
        max_time: Option<f64>,
    ) -> PyResult<Self> {
        let runtime = pyo3_async_runtimes::tokio::get_runtime();
        runtime.block_on(async {
            let token = component.inner.drt().runtime().child_token();

            // Create a temp path if none provided
            let path = match output_path {
                Some(p) => p,
                None => {
                    let temp_dir = std::env::temp_dir();
                    temp_dir
                        .join("kv_events.jsonl")
                        .to_string_lossy()
                        .to_string()
                }
            };

            let inner = llm_rs::kv_router::recorder::KvRecorder::new(
                token.clone(),
                path,
                max_lines_per_file,
                max_count,
                max_time,
            )
            .await
            .map_err(to_pyerr)?;

            // Subscribe to KV events
            let mut kv_events_rx = component
                .inner
                .subscribe(llm_rs::kv_router::KV_EVENT_SUBJECT)
                .await
                .map_err(to_pyerr)?;
            let event_tx = inner.event_sender();

            // Spawn a task to forward events to the recorder
            tokio::spawn(async move {
                while let Some(event) = kv_events_rx.next().await {
                    let event: llm_rs::kv_router::indexer::RouterEvent =
                        serde_json::from_slice(&event.payload).unwrap();
                    tracing::debug!("KvRecorder received kv event: {:?}", event);
                    if let Err(e) = event_tx.send(event).await {
                        tracing::trace!(
                            "KvRecorder failed to send kv event; shutting down: {:?}",
                            e
                        );
                    }
                }
            });

            Ok(Self {
                inner: Arc::new(inner),
            })
        })
    }

    fn event_count<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let recorder = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let count = recorder.event_count().await;
            Ok(count)
        })
    }

    fn elapsed_time<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let recorder = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            match recorder.elapsed_time().await {
                Ok(elapsed) => Ok(elapsed.as_secs_f64()),
                Err(_) => Ok(0.0), // Return 0.0 when no events have been received yet
            }
        })
    }

    #[pyo3(signature = (indexer, timed=false, max_count=None, max_time=None))]
    fn replay_events<'py>(
        &self,
        py: Python<'py>,
        indexer: &KvIndexer,
        timed: bool,
        max_count: Option<usize>,
        max_time: Option<f64>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let event_tx = indexer.inner.event_sender();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let count = llm_rs::kv_router::recorder::KvRecorder::send_events(
                "dummy_path", // This doesn't matter as we'll use the provided event_tx
                &event_tx,
                timed,
                max_count,
                max_time,
            )
            .await
            .map_err(to_pyerr)?;
            Ok(count)
        })
    }

    fn shutdown(&self) -> PyResult<()> {
        self.inner.shutdown();
        Ok(())
    }
}

#[pyclass]
#[repr(transparent)]
pub struct ForwardPassMetrics(pub RsForwardPassMetrics);

#[pyclass]
#[repr(transparent)]
pub struct WorkerStats(pub RsWorkerStats);

#[pyclass]
#[repr(transparent)]
pub struct KvStats(pub RsKvStats);

#[pyclass]
#[repr(transparent)]
pub struct SpecDecodeStats(pub RsSpecDecodeStats);

#[pymethods]
impl ForwardPassMetrics {
    #[new]
    #[pyo3(signature = (worker_stats, kv_stats, spec_decode_stats = None))]
    fn new(
        worker_stats: &WorkerStats,
        kv_stats: &KvStats,
        spec_decode_stats: Option<&SpecDecodeStats>,
    ) -> Self {
        Self(RsForwardPassMetrics {
            worker_stats: worker_stats.0.clone(),
            kv_stats: kv_stats.0.clone(),
            spec_decode_stats: spec_decode_stats.map(|s| s.0.clone()),
        })
    }
}

#[pymethods]
impl WorkerStats {
    #[new]
    #[pyo3(signature = (request_active_slots, request_total_slots, num_requests_waiting, data_parallel_rank=None))]
    fn new(
        request_active_slots: u64,
        request_total_slots: u64,
        num_requests_waiting: u64,
        data_parallel_rank: Option<DpRank>,
    ) -> Self {
        Self(RsWorkerStats {
            data_parallel_rank,
            request_active_slots,
            request_total_slots,
            num_requests_waiting,
        })
    }
}

#[pymethods]
impl KvStats {
    #[new]
    #[pyo3(signature = (kv_active_blocks, kv_total_blocks, gpu_cache_usage_perc, gpu_prefix_cache_hit_rate))]
    fn new(
        kv_active_blocks: u64,
        kv_total_blocks: u64,
        gpu_cache_usage_perc: f32,
        gpu_prefix_cache_hit_rate: f32,
    ) -> Self {
        Self(RsKvStats {
            kv_active_blocks,
            kv_total_blocks,
            gpu_cache_usage_perc,
            gpu_prefix_cache_hit_rate,
        })
    }
}

#[pymethods]
impl SpecDecodeStats {
    #[new]
    #[pyo3(signature = (num_spec_tokens, num_drafts, num_draft_tokens, num_accepted_tokens, num_accepted_tokens_per_pos))]
    fn new(
        num_spec_tokens: Option<u32>,
        num_drafts: Option<u32>,
        num_draft_tokens: Option<u32>,
        num_accepted_tokens: Option<u32>,
        num_accepted_tokens_per_pos: Option<Vec<u32>>,
    ) -> Self {
        Self(RsSpecDecodeStats {
            num_spec_tokens,
            num_drafts,
            num_draft_tokens,
            num_accepted_tokens,
            num_accepted_tokens_per_pos,
        })
    }
}

/// Helper function to create a KV router from an endpoint using the ModelManager
/// to ensure proper etcd registration
async fn create_kv_router_from_endpoint(
    endpoint: &Endpoint,
    block_size: usize,
    kv_router_config: Option<llm_rs::kv_router::KvRouterConfig>,
) -> Result<Arc<llm_rs::kv_router::KvRouter>, PyErr> {
    // Get component from endpoint
    let component = endpoint.inner.component();

    // Verify we're not in static mode
    if component.drt().primary_lease().is_none() {
        return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
            "Failed to get primary lease: Cannot KV route static workers",
        ));
    }

    // Create ModelManager and use it to create KvRouter (ensures etcd registration)
    let model_manager = Arc::new(llm_rs::discovery::ModelManager::new());
    let kv_router = model_manager
        .kv_chooser_for(
            "dummy_name", // does not matter, never cached
            component,
            block_size as u32,
            kv_router_config,
        )
        .await
        .map_err(to_pyerr)?;

    Ok(kv_router)
}

#[pyclass]
pub(crate) struct KvPushRouter {
    inner: Arc<llm_rs::kv_router::KvPushRouter>,
}

// TODO: can this reuse the stream conversion method in Client bindings?
impl KvPushRouter {
    /// Helper method to process a request and create a Python async generator
    fn process_request_to_stream<'p>(
        py: Python<'p>,
        inner: Arc<llm_rs::kv_router::KvPushRouter>,
        request: llm_rs::protocols::common::preprocessor::PreprocessedRequest,
    ) -> PyResult<Bound<'p, PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let single_in = SingleIn::new(request);
            let stream = inner.generate(single_in).await.map_err(to_pyerr)?;
            let (tx, rx) = tokio::sync::mpsc::channel(100);

            // Spawn a task to process the stream
            tokio::spawn(async move {
                let mut stream = stream;
                while let Some(response) = stream.next().await {
                    // Convert LLMEngineOutput to PyObject
                    let py_response = Python::with_gil(|py| {
                        pythonize(py, &response.data)
                            .map(|obj| obj.unbind())
                            .map_err(|e| e.to_string())
                    });

                    match py_response {
                        Ok(obj) => {
                            if tx.send(obj).await.is_err() {
                                break; // Receiver dropped
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to pythonize response: {}", e);
                            break;
                        }
                    }
                }
            });

            // Return a Python async generator wrapper
            Ok(KvPushRouterStream {
                rx: Arc::new(tokio::sync::Mutex::new(rx)),
            })
        })
    }
}

#[pymethods]
impl KvPushRouter {
    #[new]
    fn new(
        endpoint: &Endpoint,
        block_size: usize,
        kv_router_config: &super::entrypoint::KvRouterConfig,
    ) -> PyResult<Self> {
        let runtime = pyo3_async_runtimes::tokio::get_runtime();
        runtime.block_on(async move {
            let client = endpoint.inner.client().await.map_err(to_pyerr)?;

            // Create PushRouter with KV router mode
            let push_router = rs::pipeline::PushRouter::<
                llm_rs::protocols::common::preprocessor::PreprocessedRequest,
                rs::protocols::annotated::Annotated<
                    llm_rs::protocols::common::llm_backend::LLMEngineOutput,
                >,
            >::from_client(
                client,
                rs::pipeline::network::egress::push_router::RouterMode::KV,
            )
            .await
            .map_err(to_pyerr)?;

            // Create KvRouter using helper function (ensures etcd registration)
            let kv_router = create_kv_router_from_endpoint(
                endpoint,
                block_size,
                Some(kv_router_config.inner()),
            )
            .await?;

            // Create KvPushRouter (kv_router is already Arc<KvRouter>)
            let kv_push_router = llm_rs::kv_router::KvPushRouter::new(push_router, kv_router);

            Ok(Self {
                inner: Arc::new(kv_push_router),
            })
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (token_ids, model, stop_conditions=None, sampling_options=None, output_options=None, router_config_override=None, worker_id=None, dp_rank=None, extra_args=None))]
    fn generate<'p>(
        &self,
        py: Python<'p>,
        token_ids: Vec<u32>,
        model: String,
        stop_conditions: Option<PyObject>,
        sampling_options: Option<PyObject>,
        output_options: Option<PyObject>,
        router_config_override: Option<PyObject>,
        worker_id: Option<WorkerId>,
        dp_rank: Option<DpRank>,
        extra_args: Option<PyObject>,
    ) -> PyResult<Bound<'p, PyAny>> {
        // Depythonize the options with defaults
        let (stop_conditions, sampling_options, output_options, router_config_override, extra_args) =
            Python::with_gil(|py| {
                let stop_conditions: StopConditions = if let Some(obj) = stop_conditions {
                    depythonize(obj.bind(py)).map_err(to_pyerr)?
                } else {
                    StopConditions::default()
                };

                let sampling_options: SamplingOptions = if let Some(obj) = sampling_options {
                    depythonize(obj.bind(py)).map_err(to_pyerr)?
                } else {
                    SamplingOptions::default()
                };

                let output_options: OutputOptions = if let Some(obj) = output_options {
                    depythonize(obj.bind(py)).map_err(to_pyerr)?
                } else {
                    OutputOptions::default()
                };

                let router_config_override: Option<llm_rs::kv_router::RouterConfigOverride> =
                    if let Some(obj) = router_config_override {
                        Some(depythonize(obj.bind(py)).map_err(to_pyerr)?)
                    } else {
                        None
                    };

                let extra_args: Option<serde_json::Value> = if let Some(obj) = extra_args {
                    Some(depythonize(obj.bind(py)).map_err(to_pyerr)?)
                } else {
                    None
                };

                Ok::<_, PyErr>((
                    stop_conditions,
                    sampling_options,
                    output_options,
                    router_config_override,
                    extra_args,
                ))
            })?;

        // Build the PreprocessedRequest
        let mut request_builder =
            llm_rs::protocols::common::preprocessor::PreprocessedRequest::builder();
        request_builder
            .model(model)
            .token_ids(token_ids)
            .stop_conditions(stop_conditions)
            .sampling_options(sampling_options)
            .output_options(output_options)
            .router_config_override(router_config_override)
            .dp_rank(dp_rank)
            .extra_args(extra_args);

        // Set backend_instance_id if worker_id is provided
        if let Some(worker_id) = worker_id {
            request_builder.backend_instance_id(Some(worker_id));
        }

        let request = request_builder.build().map_err(to_pyerr)?;

        // Use the helper method to process the request
        Self::process_request_to_stream(py, self.inner.clone(), request)
    }

    fn generate_from_request<'p>(
        &self,
        py: Python<'p>,
        request: PyObject,
    ) -> PyResult<Bound<'p, PyAny>> {
        // Depythonize the request directly into PreprocessedRequest
        let request: llm_rs::protocols::common::preprocessor::PreprocessedRequest =
            Python::with_gil(|py| depythonize(request.bind(py)).map_err(to_pyerr))?;

        // Use the helper method to process the request
        Self::process_request_to_stream(py, self.inner.clone(), request)
    }

    #[pyo3(signature = (token_ids, router_config_override=None, request_id=None))]
    fn best_worker<'p>(
        &self,
        py: Python<'p>,
        token_ids: Vec<u32>,
        router_config_override: Option<PyObject>,
        request_id: Option<String>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let router_config_override = if let Some(obj) = router_config_override {
            Python::with_gil(|py| {
                let override_config: llm_rs::kv_router::RouterConfigOverride =
                    depythonize(obj.bind(py)).map_err(to_pyerr)?;
                Ok::<_, PyErr>(Some(override_config))
            })?
        } else {
            None
        };

        let chooser = self.inner.chooser.clone();
        let update_states = request_id.is_some();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (best_worker, overlap_blocks) = chooser
                .find_best_match(
                    request_id.as_deref(),
                    &token_ids,
                    router_config_override.as_ref(),
                    update_states,
                )
                .await
                .map_err(to_pyerr)?;

            Ok((best_worker.worker_id, best_worker.dp_rank, overlap_blocks))
        })
    }

    /// Deprecated: Use `best_worker()` instead which returns (worker_id, dp_rank, overlap_blocks)
    #[pyo3(signature = (token_ids, router_config_override=None, request_id=None))]
    fn best_worker_id<'p>(
        &self,
        py: Python<'p>,
        token_ids: Vec<u32>,
        router_config_override: Option<PyObject>,
        request_id: Option<String>,
    ) -> PyResult<Bound<'p, PyAny>> {
        // Issue deprecation warning
        let warnings = py.import("warnings")?;
        warnings.call_method1(
            "warn",
            (
                "best_worker_id() is deprecated. Use best_worker() instead which returns (worker_id, dp_rank, overlap_blocks)",
                py.get_type::<pyo3::exceptions::PyDeprecationWarning>(),
            ),
        )?;

        let router_config_override = if let Some(obj) = router_config_override {
            Python::with_gil(|py| {
                let override_config: llm_rs::kv_router::RouterConfigOverride =
                    depythonize(obj.bind(py)).map_err(to_pyerr)?;
                Ok::<_, PyErr>(Some(override_config))
            })?
        } else {
            None
        };

        let chooser = self.inner.chooser.clone();
        let update_states = request_id.is_some();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (best_worker, overlap_blocks) = chooser
                .find_best_match(
                    request_id.as_deref(),
                    &token_ids,
                    router_config_override.as_ref(),
                    update_states,
                )
                .await
                .map_err(to_pyerr)?;

            // Return only worker_id and overlap_blocks for backward compatibility
            Ok((best_worker.worker_id, overlap_blocks))
        })
    }

    /// Mark prefill as completed for a request
    fn mark_prefill_complete<'p>(
        &self,
        py: Python<'p>,
        request_id: String,
    ) -> PyResult<Bound<'p, PyAny>> {
        let chooser = self.inner.chooser.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            chooser
                .mark_prefill_completed(&request_id)
                .await
                .map_err(to_pyerr)?;
            Ok(())
        })
    }

    /// Free a request by its ID, signaling the router to release resources
    fn free<'p>(&self, py: Python<'p>, request_id: String) -> PyResult<Bound<'p, PyAny>> {
        let chooser = self.inner.chooser.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            chooser.free(&request_id).await.map_err(to_pyerr)?;
            Ok(())
        })
    }

    fn get_potential_loads<'p>(
        &self,
        py: Python<'p>,
        token_ids: Vec<u32>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let chooser = self.inner.chooser.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let loads = chooser
                .get_potential_loads(&token_ids)
                .await
                .map_err(to_pyerr)?;

            // Return loads without aggregation - each (worker_id, dp_rank) pair is a separate entry
            // Use pythonize to convert Vec<PotentialLoad> to Python list of dicts
            Python::with_gil(|py| {
                pythonize(py, &loads)
                    .map(|obj| obj.unbind())
                    .map_err(to_pyerr)
            })
        })
    }

    /// Dump all events from the KV router's indexer as a JSON string
    fn dump_events<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        let chooser = self.inner.chooser.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let events = chooser.dump_events().await.map_err(to_pyerr)?;
            // Serialize to JSON string
            let json_str = serde_json::to_string(&events).map_err(to_pyerr)?;
            Ok(json_str)
        })
    }
}

// Python async generator wrapper for the stream
#[pyclass]
pub(crate) struct KvPushRouterStream {
    rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<PyObject>>>,
}

#[pymethods]
impl KvPushRouterStream {
    #[pyo3(name = "__aiter__")]
    fn aiter(slf: Bound<'_, Self>) -> PyResult<Py<PyAny>> {
        Ok(slf.clone().into_any().unbind())
    }

    #[pyo3(name = "__anext__")]
    fn anext<'p>(&self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        let rx = self.rx.clone();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut rx = rx.lock().await;
            match rx.recv().await {
                Some(obj) => Ok(obj),
                None => Err(pyo3::exceptions::PyStopAsyncIteration::new_err(
                    "Stream exhausted",
                )),
            }
        })
    }
}

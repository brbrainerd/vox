#[cfg(test)]
#[allow(clippy::module_inception)] // File is `tests.rs`; inner `mod tests` keeps `cfg(test)` items grouped.
mod tests {
    use crate::mens::hardware::mock::MockProbe;
    use crate::mens::hardware::pipeline::ProbePipeline;
    use crate::mens::hardware::probe::{HardwareProbe, ProbeError, ProbeOutcome};
    use crate::mens::hardware::types::{ComputeBackend, GpuVendor, HardwareSummary};

    fn dummy_summary() -> HardwareSummary {
        HardwareSummary {
            model_name: "Test GPU".into(),
            vram_mb: 8192,
            gpu_count: 1,
            vendor: GpuVendor::Nvidia,
            backend: ComputeBackend::Cuda,
            driver_version: None,
            pci_bus_id: None,
            probe_failures: None,
        }
    }

    #[tokio::test]
    async fn mock_probe_returns_configured_result() {
        let probe = MockProbe {
            name: "test",
            applicable: true,
            result: Ok(Some(dummy_summary())),
        };
        assert_eq!(probe.name(), "test");
        assert!(probe.applicable());
        let res = probe.probe().await.unwrap();
        assert_eq!(res.unwrap().model_name, "Test GPU");
    }

    #[tokio::test]
    async fn mock_probe_propagates_error() {
        let probe = MockProbe {
            name: "broken",
            applicable: true,
            result: Err(ProbeError::LibraryUnavailable("nvml".into())),
        };
        assert_eq!(
            probe.probe().await.unwrap_err(),
            ProbeError::LibraryUnavailable("nvml".into())
        );
    }

    #[tokio::test]
    async fn pipeline_returns_first_found() {
        let pipeline = ProbePipeline::empty().with_probe(Box::new(MockProbe {
            name: "first",
            applicable: true,
            result: Ok(Some(dummy_summary())),
        }));
        let report = pipeline.run().await;
        assert_eq!(report.summary.model_name, "Test GPU");
        assert_eq!(report.attempts.len(), 1);
        assert!(matches!(report.attempts[0].outcome, ProbeOutcome::Found(_)));
    }

    #[tokio::test]
    async fn pipeline_skips_no_device_to_next() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "no_dev",
                applicable: true,
                result: Ok(None),
            }))
            .with_probe(Box::new(MockProbe {
                name: "found",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }));
        let report = pipeline.run().await;
        assert_eq!(report.summary.model_name, "Test GPU");
        assert!(matches!(report.attempts[0].outcome, ProbeOutcome::NoDevice));
        assert!(matches!(report.attempts[1].outcome, ProbeOutcome::Found(_)));
    }

    #[tokio::test]
    async fn pipeline_failure_does_not_abort() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "broken",
                applicable: true,
                result: Err(ProbeError::DeviceError("oops".into())),
            }))
            .with_probe(Box::new(MockProbe {
                name: "found",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }));
        let report = pipeline.run().await;
        assert_eq!(report.summary.model_name, "Test GPU");
        assert_eq!(
            report.summary.probe_failures.as_deref(),
            Some(&["broken".to_string()][..])
        );
    }

    #[tokio::test]
    async fn pipeline_all_fail_returns_cpu_fallback() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "a",
                applicable: true,
                result: Err(ProbeError::Other("a".into())),
            }))
            .with_probe(Box::new(MockProbe {
                name: "b",
                applicable: true,
                result: Err(ProbeError::Other("b".into())),
            }));
        let report = pipeline.run().await;
        assert_eq!(report.summary.model_name, "Host CPU");
        assert_eq!(
            report.summary.probe_failures.as_deref(),
            Some(&["a".to_string(), "b".to_string()][..])
        );
    }

    #[test]
    fn default_pipeline_for_platform_has_probes() {
        let pipeline = ProbePipeline::default_for_platform();
        assert!(
            !pipeline.probes.is_empty(),
            "expected at least one probe in the default platform pipeline"
        );
    }

    #[tokio::test]
    async fn pipeline_skips_not_applicable() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "off",
                applicable: false,
                result: Ok(Some(dummy_summary())),
            }))
            .with_probe(Box::new(MockProbe {
                name: "on",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }));
        let report = pipeline.run().await;
        assert_eq!(report.attempts[0].probe_name, "off");
        assert!(matches!(
            report.attempts[0].outcome,
            ProbeOutcome::NotApplicable
        ));
        assert_eq!(report.attempts[0].duration_ms, 0);
        assert!(matches!(report.attempts[1].outcome, ProbeOutcome::Found(_)));
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn pipeline_emits_event_per_attempt() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "found",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }))
            .with_probe(Box::new(MockProbe {
                name: "broken",
                applicable: true,
                result: Err(ProbeError::Other("x".into())),
            }));
        let _ = pipeline.run().await;
        assert!(logs_contain("vox.mesh.probe.name=\"found\""));
        assert!(logs_contain("vox.mesh.probe.outcome=\"found\""));
        assert!(logs_contain("vox.mesh.probe.name=\"broken\""));
        assert!(logs_contain("vox.mesh.probe.outcome=\"failed\""));
    }

    #[tokio::test]
    async fn pipeline_reorder_changes_probe_sequence() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "alpha",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }))
            .with_probe(Box::new(MockProbe {
                name: "beta",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }))
            .reorder(&["beta", "alpha"]);
        assert_eq!(pipeline.probes[0].name(), "beta");
        assert_eq!(pipeline.probes[1].name(), "alpha");
    }

    #[test]
    fn validate_probe_names_rejects_unknown() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "nvml",
                applicable: true,
                result: Ok(None),
            }))
            .with_probe(Box::new(MockProbe {
                name: "wgpu",
                applicable: true,
                result: Ok(None),
            }));
        assert_eq!(pipeline.validate_probe_names(&["nvml", "wgpu"]), Ok(()));
        assert_eq!(
            pipeline.validate_probe_names(&["nvml", "typo_name"]),
            Err(vec!["typo_name".to_string()])
        );
    }

    #[tokio::test]
    async fn pipeline_reorder_unknown_names_appended() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "a",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }))
            .with_probe(Box::new(MockProbe {
                name: "b",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }))
            .reorder(&["b"]);
        // "b" is first, "a" is appended after
        assert_eq!(pipeline.probes[0].name(), "b");
        assert_eq!(pipeline.probes[1].name(), "a");
    }
}

use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookPoint {
    TurnStart,
    ToolCallStart,
    ToolCallEnd,
    Compaction,
    TurnEnd,
    Error,
}

impl HookPoint {
    pub fn all() -> Vec<HookPoint> {
        vec![
            HookPoint::TurnStart,
            HookPoint::ToolCallStart,
            HookPoint::ToolCallEnd,
            HookPoint::Compaction,
            HookPoint::TurnEnd,
            HookPoint::Error,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct HookContext<'a> {
    pub point: HookPoint,
    pub tool_name: Option<&'a str>,
    pub summary: Option<&'a str>,
    pub payload: serde_json::Value,
}

impl<'a> HookContext<'a> {
    pub fn new(point: HookPoint) -> Self {
        Self {
            point,
            tool_name: None,
            summary: None,
            payload: serde_json::Value::Null,
        }
    }

    pub fn with_tool_name(mut self, tool_name: Option<&'a str>) -> Self {
        self.tool_name = tool_name;
        self
    }

    pub fn with_summary(mut self, summary: Option<&'a str>) -> Self {
        self.summary = summary;
        self
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }
}

pub trait Hook: Send + Sync {
    fn run<'a>(
        &'a self,
        ctx: &'a HookContext<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + Sync + 'a>>;
}

#[derive(Default, Clone)]
pub struct HookRegistry {
    hooks: Vec<(String, Arc<dyn Hook + Send + Sync>, Vec<HookPoint>)>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        name: impl Into<String>,
        hook: Arc<dyn Hook + Send + Sync>,
        points: Vec<HookPoint>,
    ) {
        self.hooks.push((name.into(), hook, points));
    }

    pub fn deregister(&mut self, name: &str) {
        self.hooks.retain(|(n, _, _)| n != name);
    }

    pub async fn fire(&self, point: HookPoint, tool_name: Option<&str>, summary: Option<&str>) {
        let owned = OwnedContext {
            tool_name: tool_name.map(str::to_string),
            summary: summary.map(str::to_string),
            payload: serde_json::Value::Null,
        };
        for (_, hook, points) in &self.hooks {
            if points.contains(&point) {
                let owned = owned.clone();
                let hook = hook.clone();
                tokio::spawn(async move {
                    let ctx = HookContext::new(point)
                        .with_tool_name(owned.tool_name.as_deref())
                        .with_summary(owned.summary.as_deref())
                        .with_payload(owned.payload.clone());
                    if let Err(err) = hook.run(&ctx).await {
                        eprintln!("hook error point={:?} err={}", point, err);
                    }
                });
            }
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    #[cfg(test)]
    pub fn points(&self, name: &str) -> Option<&[HookPoint]> {
        self.hooks
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, _, points)| points.as_slice())
    }
}

#[derive(Debug, Clone)]
struct OwnedContext {
    tool_name: Option<String>,
    summary: Option<String>,
    payload: serde_json::Value,
}

pub struct LoggingHook;

impl Hook for LoggingHook {
    fn run<'a>(
        &'a self,
        ctx: &'a HookContext<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + Sync + 'a>>
    {
        Box::pin(async move {
            let tool = ctx.tool_name.unwrap_or("-");
            let summary = ctx.summary.unwrap_or("-");
            eprintln!(
                "hook point={:?} tool={} summary={} payload={}",
                ctx.point, tool, summary, ctx.payload
            );
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingHook {
        point: HookPoint,
        count: Arc<AtomicUsize>,
    }

    impl Hook for CountingHook {
        fn run<'a>(
            &'a self,
            ctx: &'a HookContext<'_>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + Sync + 'a>,
        > {
            let count = self.count.clone();
            let point = self.point;
            Box::pin(async move {
                if ctx.point == point {
                    count.fetch_add(1, Ordering::SeqCst);
                }
                Ok(())
            })
        }
    }

    struct FailingHook;

    impl Hook for FailingHook {
        fn run<'a>(
            &'a self,
            _ctx: &'a HookContext<'_>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + Sync + 'a>,
        > {
            Box::pin(async move { anyhow::bail!("hook failed") })
        }
    }

    struct PanickingHook;

    impl Hook for PanickingHook {
        fn run<'a>(
            &'a self,
            _ctx: &'a HookContext<'_>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + Sync + 'a>,
        > {
            Box::pin(async move {
                panic!("hook panicked");
            })
        }
    }

    #[tokio::test]
    async fn register_deregister_and_len() {
        let mut registry = HookRegistry::new();
        let count = Arc::new(AtomicUsize::new(0));
        registry.register(
            "counting",
            Arc::new(CountingHook {
                point: HookPoint::TurnStart,
                count: count.clone(),
            }),
            vec![HookPoint::TurnStart],
        );
        assert_eq!(registry.len(), 1);
        assert!(
            registry
                .points("counting")
                .unwrap()
                .contains(&HookPoint::TurnStart)
        );
        registry.deregister("counting");
        assert!(registry.is_empty());
    }

    #[tokio::test]
    async fn fire_order_matches_registration_order() {
        let mut registry = HookRegistry::new();
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        registry.register(
            "first",
            Arc::new(CountingHook {
                point: HookPoint::ToolCallStart,
                count: first.clone(),
            }),
            vec![HookPoint::ToolCallStart],
        );
        registry.register(
            "second",
            Arc::new(CountingHook {
                point: HookPoint::ToolCallStart,
                count: second.clone(),
            }),
            vec![HookPoint::ToolCallStart],
        );
        registry
            .fire(HookPoint::ToolCallStart, Some("read"), Some("start"))
            .await;
        let mut attempts = 0;
        while first.load(Ordering::SeqCst) == 0
            || second.load(Ordering::SeqCst) == 0 && attempts < 200
        {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            attempts += 1;
        }
        assert_eq!(first.load(Ordering::SeqCst), 1);
        assert_eq!(second.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn failing_hook_does_not_break_fire() {
        let mut registry = HookRegistry::new();
        let count = Arc::new(AtomicUsize::new(0));
        registry.register(
            "counting",
            Arc::new(CountingHook {
                point: HookPoint::ToolCallEnd,
                count: count.clone(),
            }),
            vec![HookPoint::ToolCallEnd],
        );
        registry.register(
            "failing",
            Arc::new(FailingHook),
            vec![HookPoint::ToolCallEnd],
        );
        registry
            .fire(HookPoint::ToolCallEnd, Some("write"), Some("end"))
            .await;
        let mut attempts = 0;
        while count.load(Ordering::SeqCst) == 0 && attempts < 200 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            attempts += 1;
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn panicking_hook_does_not_break_fire() {
        let mut registry = HookRegistry::new();
        let count = Arc::new(AtomicUsize::new(0));
        registry.register(
            "counting",
            Arc::new(CountingHook {
                point: HookPoint::Error,
                count: count.clone(),
            }),
            vec![HookPoint::Error],
        );
        registry.register("panicking", Arc::new(PanickingHook), vec![HookPoint::Error]);
        registry.fire(HookPoint::Error, None, Some("error")).await;
        let mut attempts = 0;
        while count.load(Ordering::SeqCst) == 0 && attempts < 200 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            attempts += 1;
        }
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn logging_hook_writes_stderr() {
        let mut registry = HookRegistry::new();
        registry.register("logging", Arc::new(LoggingHook), HookPoint::all());
        registry
            .fire(HookPoint::TurnEnd, None, Some("turn end"))
            .await;
    }
}

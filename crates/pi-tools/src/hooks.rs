use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::task::JoinSet;

static GLOBAL_HOOK_REGISTRY: OnceLock<Mutex<Option<Arc<HookRegistry>>>> = OnceLock::new();

pub fn global_hook_registry() -> Option<Arc<HookRegistry>> {
    GLOBAL_HOOK_REGISTRY
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub fn set_global_hook_registry(registry: Option<Arc<HookRegistry>>) {
    let lock = GLOBAL_HOOK_REGISTRY.get_or_init(|| Mutex::new(None));
    *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = registry;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LifecycleEvent {
    ToolCallStarted {
        name: String,
        args_json: String,
    },
    ToolCallFinished {
        name: String,
        is_error: bool,
        duration_ms: u64,
    },
    TurnStarted {
        prompt: String,
    },
    TurnFinished {
        ok: bool,
    },
}

pub trait Hook: Send + Sync {
    fn on_event<'a>(
        &'a self,
        event: &'a LifecycleEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

#[derive(Default)]
pub struct HookRegistry {
    hooks: Mutex<Vec<(usize, Arc<dyn Hook>)>>,
    enabled: AtomicBool,
    next_id: Mutex<usize>,
}

impl std::fmt::Debug for HookRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRegistry")
            .field("enabled", &self.enabled.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl HookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, hook: Arc<dyn Hook>) -> usize {
        let mut next = self.next_id.lock().unwrap_or_else(|p| p.into_inner());
        let id = *next;
        *next += 1;
        self.hooks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((id, hook));
        id
    }

    pub async fn emit(&self, event: &LifecycleEvent) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }

        let hooks: Vec<(usize, Arc<dyn Hook>)> = {
            let guard = self.hooks.lock().unwrap_or_else(|p| p.into_inner());
            guard.clone()
        };
        if hooks.is_empty() {
            return;
        }

        let event = event.clone();
        let mut set = JoinSet::new();
        for (_id, hook) in hooks {
            let event = event.clone();
            set.spawn(async move {
                hook.on_event(&event).await;
            });
        }
        while set.join_next().await.is_some() {}
    }

    pub fn enable(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingHook {
        counter: Arc<AtomicUsize>,
        events: Arc<Mutex<Vec<LifecycleEvent>>>,
        panic_on: Option<&'static str>,
    }

    impl Hook for CountingHook {
        fn on_event<'a>(
            &'a self,
            event: &'a LifecycleEvent,
        ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
            let counter = self.counter.clone();
            let events = self.events.clone();
            let panic_on = self.panic_on;
            let event = event.clone();
            Box::pin(async move {
                if let Some(expected) = panic_on {
                    let text = format!("{:?}", event);
                    if text.contains(expected) {
                        panic!("hook panic on {}", expected);
                    }
                }
                counter.fetch_add(1, Ordering::SeqCst);
                events.lock().expect("events lock poisoned").push(event);
            })
        }
    }

    #[tokio::test]
    async fn hook_receives_events_in_order() {
        let counter = Arc::new(AtomicUsize::new(0));
        let events = Arc::new(Mutex::new(Vec::new()));
        let registry = HookRegistry::new();
        registry.enable(true);
        registry.register(Arc::new(CountingHook {
            counter: counter.clone(),
            events: events.clone(),
            panic_on: None,
        }));

        registry
            .emit(&LifecycleEvent::TurnStarted {
                prompt: "hello".into(),
            })
            .await;
        registry
            .emit(&LifecycleEvent::ToolCallStarted {
                name: "read".into(),
                args_json: "{}".into(),
            })
            .await;
        registry
            .emit(&LifecycleEvent::ToolCallFinished {
                name: "read".into(),
                is_error: false,
                duration_ms: 1,
            })
            .await;
        registry
            .emit(&LifecycleEvent::TurnFinished { ok: true })
            .await;

        assert_eq!(counter.load(Ordering::SeqCst), 4);
        let events = events.lock().expect("events lock poisoned");
        assert_eq!(
            events[0],
            LifecycleEvent::TurnStarted {
                prompt: "hello".into()
            }
        );
        assert_eq!(
            events[1],
            LifecycleEvent::ToolCallStarted {
                name: "read".into(),
                args_json: "{}".into(),
            }
        );
        assert_eq!(
            events[2],
            LifecycleEvent::ToolCallFinished {
                name: "read".into(),
                is_error: false,
                duration_ms: 1,
            }
        );
        assert_eq!(events[3], LifecycleEvent::TurnFinished { ok: true });
    }

    #[tokio::test]
    async fn hook_panic_does_not_break_emission() {
        let counter = Arc::new(AtomicUsize::new(0));
        let events = Arc::new(Mutex::new(Vec::new()));
        let registry = HookRegistry::new();
        registry.enable(true);
        registry.register(Arc::new(CountingHook {
            counter: counter.clone(),
            events: events.clone(),
            panic_on: Some("bad"),
        }));
        registry.register(Arc::new(CountingHook {
            counter: counter.clone(),
            events,
            panic_on: None,
        }));

        registry
            .emit(&LifecycleEvent::ToolCallFinished {
                name: "bad".into(),
                is_error: true,
                duration_ms: 0,
            })
            .await;
        registry
            .emit(&LifecycleEvent::TurnFinished { ok: false })
            .await;

        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn empty_registry_is_free() {
        let registry = HookRegistry::new();
        registry.enable(true);
        registry
            .emit(&LifecycleEvent::TurnStarted {
                prompt: String::new(),
            })
            .await;
        registry
            .emit(&LifecycleEvent::TurnFinished { ok: true })
            .await;
    }
}

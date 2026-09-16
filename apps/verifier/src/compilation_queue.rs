use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};

use crate::compilers::CompilerError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilationStatus {
    Queued,
    Compiling,
}

#[derive(Clone)]
pub struct CompilationQueue {
    slots: Option<Arc<Semaphore>>,
    statuses: Arc<Mutex<HashMap<String, CompilationCounts>>>,
}

impl CompilationQueue {
    pub fn new(max_concurrent_compilations: Option<usize>) -> Self {
        Self {
            slots: max_concurrent_compilations.map(|limit| Arc::new(Semaphore::new(limit))),
            statuses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn acquire(&self, code_hash: &str) -> Result<CompilationPermit, CompilerError> {
        let Some(slots) = &self.slots else {
            return Ok(CompilationPermit::without_semaphore(
                Arc::clone(&self.statuses),
                code_hash,
            ));
        };

        match Arc::clone(slots).try_acquire_owned() {
            Ok(permit) => Ok(CompilationPermit::acquired(
                Arc::clone(&self.statuses),
                code_hash,
                permit,
            )),
            Err(TryAcquireError::NoPermits) => {
                let mut registration = CompilationRegistration::new(
                    Arc::clone(&self.statuses),
                    code_hash,
                    CompilationStatus::Queued,
                );
                let permit = Arc::clone(slots)
                    .acquire_owned()
                    .await
                    .map_err(|_| CompilerError::ConcurrencyLimiterClosed)?;
                registration.start_compiling();
                Ok(CompilationPermit {
                    _registration: registration,
                    _semaphore_permit: Some(permit),
                })
            }
            Err(TryAcquireError::Closed) => Err(CompilerError::ConcurrencyLimiterClosed),
        }
    }

    pub fn status(&self, code_hash: &str) -> Option<CompilationStatus> {
        let statuses = lock_statuses(&self.statuses);
        statuses.get(code_hash).and_then(|counts| {
            if counts.compiling > 0 {
                Some(CompilationStatus::Compiling)
            } else if counts.queued > 0 {
                Some(CompilationStatus::Queued)
            } else {
                None
            }
        })
    }
}

#[derive(Default)]
struct CompilationCounts {
    queued: usize,
    compiling: usize,
}

struct CompilationRegistration {
    statuses: Arc<Mutex<HashMap<String, CompilationCounts>>>,
    code_hash: String,
    status: CompilationStatus,
}

impl CompilationRegistration {
    fn new(
        statuses: Arc<Mutex<HashMap<String, CompilationCounts>>>,
        code_hash: &str,
        status: CompilationStatus,
    ) -> Self {
        let mut active = lock_statuses(&statuses);
        let counts = active.entry(code_hash.to_owned()).or_default();
        match status {
            CompilationStatus::Queued => counts.queued += 1,
            CompilationStatus::Compiling => counts.compiling += 1,
        }
        drop(active);

        Self {
            statuses,
            code_hash: code_hash.to_owned(),
            status,
        }
    }

    fn start_compiling(&mut self) {
        let mut statuses = lock_statuses(&self.statuses);
        if let Some(counts) = statuses.get_mut(&self.code_hash) {
            counts.queued -= 1;
            counts.compiling += 1;
            self.status = CompilationStatus::Compiling;
        }
    }
}

impl Drop for CompilationRegistration {
    fn drop(&mut self) {
        let mut statuses = lock_statuses(&self.statuses);
        let remove = if let Some(counts) = statuses.get_mut(&self.code_hash) {
            match self.status {
                CompilationStatus::Queued => counts.queued -= 1,
                CompilationStatus::Compiling => counts.compiling -= 1,
            }
            counts.queued == 0 && counts.compiling == 0
        } else {
            false
        };
        if remove {
            statuses.remove(&self.code_hash);
        }
    }
}

pub struct CompilationPermit {
    _registration: CompilationRegistration,
    _semaphore_permit: Option<OwnedSemaphorePermit>,
}

impl CompilationPermit {
    fn acquired(
        statuses: Arc<Mutex<HashMap<String, CompilationCounts>>>,
        code_hash: &str,
        permit: OwnedSemaphorePermit,
    ) -> Self {
        Self {
            _registration: CompilationRegistration::new(
                statuses,
                code_hash,
                CompilationStatus::Compiling,
            ),
            _semaphore_permit: Some(permit),
        }
    }

    fn without_semaphore(
        statuses: Arc<Mutex<HashMap<String, CompilationCounts>>>,
        code_hash: &str,
    ) -> Self {
        Self {
            _registration: CompilationRegistration::new(
                statuses,
                code_hash,
                CompilationStatus::Compiling,
            ),
            _semaphore_permit: None,
        }
    }
}

fn lock_statuses(
    statuses: &Mutex<HashMap<String, CompilationCounts>>,
) -> MutexGuard<'_, HashMap<String, CompilationCounts>> {
    statuses
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

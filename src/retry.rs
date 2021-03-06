use humantime::format_duration;
use if_chain::if_chain;
use log::*;
use std::cmp;
use std::fmt;
use std::time::Duration;

use crate::error::Error;
use futures_retry::{ErrorHandler, RetryPolicy};

pub struct RetryErrorHandler {
    max_attempts: Option<usize>,
    delay: Duration,
    max_delay: Duration,
    attempt: usize,
    log_context: String,
    log_action: String,
}

impl RetryErrorHandler {
    pub fn new(max_attempts: Option<usize>, delay: Duration, max_delay: Duration,
            log_context: impl fmt::Display, log_action: impl fmt::Display) -> Self {
        Self {
            max_attempts,
            delay,
            max_delay,
            attempt: 0,
            log_context: log_context.to_string(),
            log_action: log_action.to_string(),
        }
    }
}

impl ErrorHandler<Error> for RetryErrorHandler {
    type OutError = Error;

    fn handle(&mut self, e: Error) -> RetryPolicy<Error> {
        if_chain! {
            if let Some(max_attempts) = self.max_attempts;
            if self.attempt == max_attempts;
            then {
                error!("[{}] {}: final attempt {} failed: {:?}",
                    self.log_context, self.log_action, max_attempts, e);
                return RetryPolicy::ForwardError(e);
            }
        }
        self.attempt += 1;
        if let Some(max_attempts) = self.max_attempts {
            warn!("[{}] {}: attempt {} of {} failed, retrying in {}: {:?}",
                self.log_context, self.log_action, self.attempt, max_attempts,
                format_duration(self.delay), e);
        } else {
            warn!("[{}] {}: attempt {} failed, retrying in {}: {:?}",
                self.log_context, self.log_action, self.attempt, format_duration(self.delay), e);
        }
        let delay = self.delay;
        self.delay = cmp::min(self.delay * 2, self.max_delay);
        RetryPolicy::WaitRetry(delay)
    }
}
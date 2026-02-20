use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApprovalWorkflowContext {
    pub request_id: Uuid,
    pub request_type: String,
    pub required_approvals: i32,
    pub current_approvals: i32,
    pub approval_mode: ApprovalModeKind,
    pub timeout_hours: i32,
    pub auto_approve_low_risk: bool,
    pub risk_level: RiskLevelKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ApprovalModeKind {
    Single,
    #[default]
    Quorum,
    Unanimous,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RiskLevelKind {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub enum ApprovalEvent {
    Submit {
        requestor_id: Uuid,
        submitted_at: DateTime<Utc>,
    },
    Approve {
        approver_id: Uuid,
        approved_at: DateTime<Utc>,
        comment: Option<String>,
    },
    Reject {
        rejector_id: Uuid,
        rejected_at: DateTime<Utc>,
        reason: String,
    },
    Expire {
        expired_at: DateTime<Utc>,
    },
    Cancel {
        cancelled_by: Uuid,
        cancelled_at: DateTime<Utc>,
    },
    Apply {
        applied_by: Uuid,
        applied_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecisionRecord {
    pub approver_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum WorkflowState {
    #[default]
    Draft,
    Pending {
        submitted_at: DateTime<Utc>,
    },
    Approved {
        approved_at: DateTime<Utc>,
    },
    Applied {
        applied_at: DateTime<Utc>,
    },
    Rejected {
        reason: String,
        rejected_at: DateTime<Utc>,
    },
    Expired {
        expired_at: DateTime<Utc>,
    },
    Cancelled {
        cancelled_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApprovalWorkflow {
    pub context: ApprovalWorkflowContext,
    pub state: WorkflowState,
    pub decisions: Vec<ApprovalDecisionRecord>,
    pub rejection_reason: Option<String>,
    pub resolution_timestamp: Option<DateTime<Utc>>,
}

impl ApprovalWorkflow {
    pub fn new(context: ApprovalWorkflowContext) -> Self {
        Self {
            context,
            state: WorkflowState::Draft,
            decisions: Vec::new(),
            rejection_reason: None,
            resolution_timestamp: None,
        }
    }

    fn should_auto_approve(&self) -> bool {
        self.context.auto_approve_low_risk && self.context.risk_level == RiskLevelKind::Low
    }

    fn is_fully_approved(&self) -> bool {
        match self.context.approval_mode {
            ApprovalModeKind::Single => self.context.current_approvals >= 1,
            ApprovalModeKind::Quorum => {
                self.context.current_approvals >= self.context.required_approvals
            }
            ApprovalModeKind::Unanimous => {
                self.context.current_approvals >= self.context.required_approvals
            }
        }
    }

    fn record_approval(
        &mut self,
        approver_id: Uuid,
        timestamp: DateTime<Utc>,
        comment: Option<String>,
    ) {
        self.decisions.push(ApprovalDecisionRecord {
            approver_id,
            timestamp,
            comment,
        });
        self.context.current_approvals += 1;
    }

    pub fn handle(&mut self, event: ApprovalEvent) -> Result<(), WorkflowError> {
        match (&self.state, event) {
            (WorkflowState::Draft, ApprovalEvent::Submit { submitted_at, .. }) => {
                if self.should_auto_approve() {
                    self.resolution_timestamp = Some(submitted_at);
                    self.state = WorkflowState::Approved {
                        approved_at: submitted_at,
                    };
                    tracing::info!(request_id = ?self.context.request_id, "Auto-approved low-risk request");
                } else {
                    self.state = WorkflowState::Pending { submitted_at };
                    tracing::info!(request_id = ?self.context.request_id, "Request submitted for approval");
                }
                Ok(())
            }

            (
                WorkflowState::Pending { .. },
                ApprovalEvent::Approve {
                    approver_id,
                    approved_at,
                    comment,
                },
            ) => {
                self.record_approval(approver_id, approved_at, comment);

                if self.is_fully_approved() {
                    self.resolution_timestamp = Some(approved_at);
                    self.state = WorkflowState::Approved { approved_at };
                    tracing::info!(
                        request_id = ?self.context.request_id,
                        approvals = self.context.current_approvals,
                        "Request fully approved"
                    );
                } else {
                    tracing::info!(
                        request_id = ?self.context.request_id,
                        current = self.context.current_approvals,
                        required = self.context.required_approvals,
                        "Approval recorded, waiting for more"
                    );
                }
                Ok(())
            }

            (
                WorkflowState::Pending { .. },
                ApprovalEvent::Reject {
                    rejected_at,
                    reason,
                    ..
                },
            ) => {
                self.rejection_reason = Some(reason.clone());
                self.resolution_timestamp = Some(rejected_at);
                self.state = WorkflowState::Rejected {
                    reason,
                    rejected_at,
                };
                tracing::info!(request_id = ?self.context.request_id, "Request rejected");
                Ok(())
            }

            (WorkflowState::Pending { .. }, ApprovalEvent::Expire { expired_at }) => {
                self.resolution_timestamp = Some(expired_at);
                self.state = WorkflowState::Expired { expired_at };
                tracing::info!(request_id = ?self.context.request_id, "Request expired");
                Ok(())
            }

            (WorkflowState::Pending { .. }, ApprovalEvent::Cancel { cancelled_at, .. }) => {
                self.resolution_timestamp = Some(cancelled_at);
                self.state = WorkflowState::Cancelled { cancelled_at };
                tracing::info!(request_id = ?self.context.request_id, "Request cancelled");
                Ok(())
            }

            (WorkflowState::Approved { .. }, ApprovalEvent::Apply { applied_at, .. }) => {
                self.state = WorkflowState::Applied { applied_at };
                tracing::info!(request_id = ?self.context.request_id, "Request applied");
                Ok(())
            }

            (current_state, event) => Err(WorkflowError::InvalidTransition {
                current_state: format!("{current_state:?}"),
                event: format!("{event:?}"),
            }),
        }
    }

    pub fn status_string(&self) -> &'static str {
        match &self.state {
            WorkflowState::Draft => "draft",
            WorkflowState::Pending { .. } => "pending",
            WorkflowState::Approved { .. } => "approved",
            WorkflowState::Applied { .. } => "applied",
            WorkflowState::Rejected { .. } => "rejected",
            WorkflowState::Expired { .. } => "expired",
            WorkflowState::Cancelled { .. } => "cancelled",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            WorkflowState::Applied { .. }
                | WorkflowState::Rejected { .. }
                | WorkflowState::Expired { .. }
                | WorkflowState::Cancelled { .. }
        )
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.state, WorkflowState::Pending { .. })
    }

    pub fn is_approved(&self) -> bool {
        matches!(
            self.state,
            WorkflowState::Approved { .. } | WorkflowState::Applied { .. }
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("Invalid transition from {current_state} with event {event}")]
    InvalidTransition {
        current_state: String,
        event: String,
    },
}

pub fn create_workflow(context: ApprovalWorkflowContext) -> ApprovalWorkflow {
    ApprovalWorkflow::new(context)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> ApprovalWorkflowContext {
        ApprovalWorkflowContext {
            request_id: Uuid::new_v4(),
            request_type: "policy".to_string(),
            required_approvals: 2,
            current_approvals: 0,
            approval_mode: ApprovalModeKind::Quorum,
            timeout_hours: 72,
            auto_approve_low_risk: false,
            risk_level: RiskLevelKind::Medium,
        }
    }

    #[test]
    fn test_submit_transitions_to_pending() {
        let mut workflow = create_workflow(test_context());

        workflow
            .handle(ApprovalEvent::Submit {
                requestor_id: Uuid::new_v4(),
                submitted_at: Utc::now(),
            })
            .unwrap();

        assert!(matches!(workflow.state, WorkflowState::Pending { .. }));
    }

    #[test]
    fn test_auto_approve_low_risk() {
        let mut ctx = test_context();
        ctx.auto_approve_low_risk = true;
        ctx.risk_level = RiskLevelKind::Low;

        let mut workflow = create_workflow(ctx);

        workflow
            .handle(ApprovalEvent::Submit {
                requestor_id: Uuid::new_v4(),
                submitted_at: Utc::now(),
            })
            .unwrap();

        assert!(matches!(workflow.state, WorkflowState::Approved { .. }));
    }

    #[test]
    fn test_quorum_approval() {
        let mut workflow = create_workflow(test_context());

        workflow
            .handle(ApprovalEvent::Submit {
                requestor_id: Uuid::new_v4(),
                submitted_at: Utc::now(),
            })
            .unwrap();

        workflow
            .handle(ApprovalEvent::Approve {
                approver_id: Uuid::new_v4(),
                approved_at: Utc::now(),
                comment: None,
            })
            .unwrap();

        assert!(matches!(workflow.state, WorkflowState::Pending { .. }));
        assert_eq!(workflow.decisions.len(), 1);

        workflow
            .handle(ApprovalEvent::Approve {
                approver_id: Uuid::new_v4(),
                approved_at: Utc::now(),
                comment: Some("LGTM".to_string()),
            })
            .unwrap();

        assert!(matches!(workflow.state, WorkflowState::Approved { .. }));
        assert_eq!(workflow.decisions.len(), 2);
    }

    #[test]
    fn test_single_approval_mode() {
        let mut ctx = test_context();
        ctx.approval_mode = ApprovalModeKind::Single;
        ctx.required_approvals = 1;

        let mut workflow = create_workflow(ctx);

        workflow
            .handle(ApprovalEvent::Submit {
                requestor_id: Uuid::new_v4(),
                submitted_at: Utc::now(),
            })
            .unwrap();

        workflow
            .handle(ApprovalEvent::Approve {
                approver_id: Uuid::new_v4(),
                approved_at: Utc::now(),
                comment: None,
            })
            .unwrap();

        assert!(matches!(workflow.state, WorkflowState::Approved { .. }));
    }

    #[test]
    fn test_rejection() {
        let mut workflow = create_workflow(test_context());

        workflow
            .handle(ApprovalEvent::Submit {
                requestor_id: Uuid::new_v4(),
                submitted_at: Utc::now(),
            })
            .unwrap();

        workflow
            .handle(ApprovalEvent::Reject {
                rejector_id: Uuid::new_v4(),
                rejected_at: Utc::now(),
                reason: "Does not meet requirements".to_string(),
            })
            .unwrap();

        assert!(matches!(workflow.state, WorkflowState::Rejected { .. }));
        assert_eq!(
            workflow.rejection_reason,
            Some("Does not meet requirements".to_string())
        );
    }

    #[test]
    fn test_expiration() {
        let mut workflow = create_workflow(test_context());

        workflow
            .handle(ApprovalEvent::Submit {
                requestor_id: Uuid::new_v4(),
                submitted_at: Utc::now(),
            })
            .unwrap();

        workflow
            .handle(ApprovalEvent::Expire {
                expired_at: Utc::now(),
            })
            .unwrap();

        assert!(matches!(workflow.state, WorkflowState::Expired { .. }));
    }

    #[test]
    fn test_cancellation() {
        let mut workflow = create_workflow(test_context());

        workflow
            .handle(ApprovalEvent::Submit {
                requestor_id: Uuid::new_v4(),
                submitted_at: Utc::now(),
            })
            .unwrap();

        workflow
            .handle(ApprovalEvent::Cancel {
                cancelled_by: Uuid::new_v4(),
                cancelled_at: Utc::now(),
            })
            .unwrap();

        assert!(matches!(workflow.state, WorkflowState::Cancelled { .. }));
    }

    #[test]
    fn test_apply_after_approval() {
        let mut ctx = test_context();
        ctx.approval_mode = ApprovalModeKind::Single;
        ctx.required_approvals = 1;

        let mut workflow = create_workflow(ctx);

        workflow
            .handle(ApprovalEvent::Submit {
                requestor_id: Uuid::new_v4(),
                submitted_at: Utc::now(),
            })
            .unwrap();

        workflow
            .handle(ApprovalEvent::Approve {
                approver_id: Uuid::new_v4(),
                approved_at: Utc::now(),
                comment: None,
            })
            .unwrap();

        workflow
            .handle(ApprovalEvent::Apply {
                applied_by: Uuid::new_v4(),
                applied_at: Utc::now(),
            })
            .unwrap();

        assert!(matches!(workflow.state, WorkflowState::Applied { .. }));
    }

    #[test]
    fn test_invalid_transition() {
        let mut workflow = create_workflow(test_context());

        let result = workflow.handle(ApprovalEvent::Approve {
            approver_id: Uuid::new_v4(),
            approved_at: Utc::now(),
            comment: None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut workflow = create_workflow(test_context());

        workflow
            .handle(ApprovalEvent::Submit {
                requestor_id: Uuid::new_v4(),
                submitted_at: Utc::now(),
            })
            .unwrap();

        workflow
            .handle(ApprovalEvent::Approve {
                approver_id: Uuid::new_v4(),
                approved_at: Utc::now(),
                comment: Some("First approval".to_string()),
            })
            .unwrap();

        let json = serde_json::to_string(&workflow).unwrap();
        let restored: ApprovalWorkflow = serde_json::from_str(&json).unwrap();

        assert_eq!(workflow.status_string(), restored.status_string());
        assert_eq!(workflow.decisions.len(), restored.decisions.len());
        assert_eq!(
            workflow.context.current_approvals,
            restored.context.current_approvals
        );
    }

    #[test]
    fn test_status_helpers() {
        let mut workflow = create_workflow(test_context());
        assert!(!workflow.is_pending());
        assert!(!workflow.is_approved());
        assert!(!workflow.is_terminal());

        workflow
            .handle(ApprovalEvent::Submit {
                requestor_id: Uuid::new_v4(),
                submitted_at: Utc::now(),
            })
            .unwrap();

        assert!(workflow.is_pending());
        assert!(!workflow.is_approved());
        assert!(!workflow.is_terminal());
    }
}

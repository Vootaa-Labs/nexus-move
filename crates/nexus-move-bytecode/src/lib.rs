#![forbid(unsafe_code)]

use nexus_move_types::ExtractionBoundary;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralLimits {
    pub max_module_count: usize,
    pub max_module_size: usize,
    pub max_total_size: usize,
}

impl Default for StructuralLimits {
    fn default() -> Self {
        Self {
            max_module_count: 64,
            max_module_size: 512 * 1024,
            max_total_size: 2 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BytecodePolicy {
    pub structural_verifier_enabled: bool,
    pub semantic_verifier_enabled: bool,
    pub limits: StructuralLimits,
}

impl BytecodePolicy {
    pub fn bootstrap() -> Self {
        Self {
            structural_verifier_enabled: true,
            semantic_verifier_enabled: false,
            limits: StructuralLimits::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationError {
    EmptyModuleSet,
    EmptyModule {
        index: usize,
    },
    TooManyModules {
        limit: usize,
        got: usize,
    },
    ModuleTooLarge {
        index: usize,
        limit: usize,
        got: usize,
    },
    TotalSizeTooLarge {
        limit: usize,
        got: usize,
    },
    DuplicateModuleBytes {
        first: usize,
        second: usize,
    },
}

pub fn planned_boundary() -> ExtractionBoundary {
    ExtractionBoundary::bootstrap()
}

pub fn verify_publish_bundle(
    modules: &[Vec<u8>],
    policy: &BytecodePolicy,
) -> Result<(), Vec<VerificationError>> {
    if !policy.structural_verifier_enabled {
        return Ok(());
    }

    let mut errors = Vec::new();

    if modules.is_empty() {
        errors.push(VerificationError::EmptyModuleSet);
    }

    if modules.len() > policy.limits.max_module_count {
        errors.push(VerificationError::TooManyModules {
            limit: policy.limits.max_module_count,
            got: modules.len(),
        });
    }

    let total_size: usize = modules.iter().map(Vec::len).sum();
    if total_size > policy.limits.max_total_size {
        errors.push(VerificationError::TotalSizeTooLarge {
            limit: policy.limits.max_total_size,
            got: total_size,
        });
    }

    for (index, module) in modules.iter().enumerate() {
        if module.is_empty() {
            errors.push(VerificationError::EmptyModule { index });
        }
        if module.len() > policy.limits.max_module_size {
            errors.push(VerificationError::ModuleTooLarge {
                index,
                limit: policy.limits.max_module_size,
                got: module.len(),
            });
        }
    }

    for second in 0..modules.len() {
        for first in 0..second {
            if modules[first] == modules[second] {
                errors.push(VerificationError::DuplicateModuleBytes { first, second });
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_publish_bundle() {
        let policy = BytecodePolicy::bootstrap();
        let errors = verify_publish_bundle(&[], &policy).unwrap_err();
        assert_eq!(errors, vec![VerificationError::EmptyModuleSet]);
    }

    #[test]
    fn rejects_duplicate_module_bytes() {
        let policy = BytecodePolicy::bootstrap();
        let modules = vec![vec![1, 2, 3], vec![1, 2, 3]];
        let errors = verify_publish_bundle(&modules, &policy).unwrap_err();
        assert!(errors.contains(&VerificationError::DuplicateModuleBytes {
            first: 0,
            second: 1,
        }));
    }

    #[test]
    fn accepts_small_unique_bundle() {
        let policy = BytecodePolicy::bootstrap();
        let modules = vec![vec![1, 2, 3], vec![4, 5, 6]];
        assert!(verify_publish_bundle(&modules, &policy).is_ok());
    }

    #[test]
    fn rejects_too_many_modules() {
        let mut policy = BytecodePolicy::bootstrap();
        policy.limits.max_module_count = 2;
        let modules = vec![vec![1], vec![2], vec![3]];
        let errors = verify_publish_bundle(&modules, &policy).unwrap_err();
        assert!(errors.contains(&VerificationError::TooManyModules { limit: 2, got: 3 }));
    }

    #[test]
    fn rejects_module_too_large() {
        let mut policy = BytecodePolicy::bootstrap();
        policy.limits.max_module_size = 4;
        let modules = vec![vec![1, 2, 3, 4, 5]];
        let errors = verify_publish_bundle(&modules, &policy).unwrap_err();
        assert!(errors.contains(&VerificationError::ModuleTooLarge {
            index: 0,
            limit: 4,
            got: 5,
        }));
    }

    #[test]
    fn rejects_total_size_too_large() {
        let mut policy = BytecodePolicy::bootstrap();
        policy.limits.max_total_size = 5;
        let modules = vec![vec![1, 2, 3], vec![4, 5, 6]];
        let errors = verify_publish_bundle(&modules, &policy).unwrap_err();
        assert!(errors.contains(&VerificationError::TotalSizeTooLarge { limit: 5, got: 6 }));
    }

    #[test]
    fn rejects_empty_module() {
        let policy = BytecodePolicy::bootstrap();
        let modules = vec![vec![1, 2, 3], vec![]];
        let errors = verify_publish_bundle(&modules, &policy).unwrap_err();
        assert!(errors.contains(&VerificationError::EmptyModule { index: 1 }));
    }

    #[test]
    fn disabled_verifier_skips_all_checks() {
        let mut policy = BytecodePolicy::bootstrap();
        policy.structural_verifier_enabled = false;
        // Would normally fail: empty set
        assert!(verify_publish_bundle(&[], &policy).is_ok());
        // Would normally fail: duplicate
        let modules = vec![vec![1, 2], vec![1, 2]];
        assert!(verify_publish_bundle(&modules, &policy).is_ok());
    }

    #[test]
    fn collects_multiple_errors_at_once() {
        let mut policy = BytecodePolicy::bootstrap();
        policy.limits.max_module_count = 1;
        policy.limits.max_total_size = 2;
        // 2 modules, both identical (3 bytes each) → TooManyModules + TotalSizeTooLarge + DuplicateModuleBytes
        let modules = vec![vec![1, 2, 3], vec![1, 2, 3]];
        let errors = verify_publish_bundle(&modules, &policy).unwrap_err();
        assert!(
            errors.len() >= 3,
            "should collect at least 3 errors, got {}",
            errors.len()
        );
    }
}

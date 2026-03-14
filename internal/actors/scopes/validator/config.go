package validator

import (
	"internal/shared/problem"
	"internal/shared/settings"
)

func ValidateConfig(config settings.AppConfig) *problem.Problem {
	if config.NATS.Enabled {
		return nil
	}
	return problem.Validation(problem.InvalidArgument, "validator config is invalid",
		problem.ValidationIssue{Field: "nats.enabled", Message: "must be true for validator"},
	)
}

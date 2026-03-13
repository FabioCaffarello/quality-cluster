package settings

import "internal/shared/problem"

const (
	cfgNotFound   problem.ProblemCode = "CFG_NOT_FOUND"
	cfgParseError problem.ProblemCode = "CFG_PARSE_ERROR"
	cfgInvalid    problem.ProblemCode = "CFG_INVALID"
)

func validationProblem(message string, issues ...problem.ValidationIssue) *problem.Problem {
	return problem.Validation(cfgInvalid, message, issues...)
}

func extractIssues(prob *problem.Problem) []problem.ValidationIssue {
	if prob == nil {
		return nil
	}

	if raw, ok := prob.Details[problem.DetailIssues]; ok {
		if issues, ok := raw.([]problem.ValidationIssue); ok {
			copied := make([]problem.ValidationIssue, len(issues))
			copy(copied, issues)
			return copied
		}
	}

	return []problem.ValidationIssue{{
		Message: prob.Message,
	}}
}

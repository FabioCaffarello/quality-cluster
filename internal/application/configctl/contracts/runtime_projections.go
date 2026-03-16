package contracts

import (
	"strings"

	"internal/shared/problem"
)

type ListActiveRuntimeProjectionsQuery struct {
	ScopeKind string `json:"scope_kind,omitempty"`
	ScopeKey  string `json:"scope_key,omitempty"`
}

func (q ListActiveRuntimeProjectionsQuery) Normalize() ListActiveRuntimeProjectionsQuery {
	q.ScopeKind = strings.ToLower(strings.TrimSpace(q.ScopeKind))
	q.ScopeKey = strings.TrimSpace(q.ScopeKey)
	return q
}

func (q ListActiveRuntimeProjectionsQuery) Validate() *problem.Problem {
	q = q.Normalize()
	if (q.ScopeKind == "") != (q.ScopeKey == "") {
		return problem.Validation(problem.InvalidArgument, "active runtime projections query is invalid", problem.ValidationIssue{
			Field:   "scope",
			Message: "scope_kind and scope_key must be provided together",
		})
	}
	return nil
}

type ListActiveRuntimeProjectionsReply struct {
	Runtimes []RuntimeProjectionRecord `json:"runtimes"`
}

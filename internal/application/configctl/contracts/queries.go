package contracts

import (
	"strings"

	"internal/shared/problem"
)

type GetConfigQuery struct {
	VersionID string `json:"version_id"`
}

func (q GetConfigQuery) Normalize() GetConfigQuery {
	q.VersionID = strings.TrimSpace(q.VersionID)
	return q
}

func (q GetConfigQuery) Validate() *problem.Problem {
	if q.VersionID != "" {
		return nil
	}

	return problem.Validation(problem.InvalidArgument, "get config query is invalid", problem.ValidationIssue{
		Field:   "version_id",
		Message: "must not be empty",
	})
}

type ListConfigsQuery struct{}

type ListActiveIngestionBindingsQuery struct {
	ScopeKind string `json:"scope_kind,omitempty"`
	ScopeKey  string `json:"scope_key,omitempty"`
}

func (q ListActiveIngestionBindingsQuery) Normalize() ListActiveIngestionBindingsQuery {
	q.ScopeKind = strings.ToLower(strings.TrimSpace(q.ScopeKind))
	q.ScopeKey = strings.TrimSpace(q.ScopeKey)
	return q
}

func (q ListActiveIngestionBindingsQuery) Validate() *problem.Problem {
	q = q.Normalize()
	if (q.ScopeKind == "") != (q.ScopeKey == "") {
		return problem.Validation(problem.InvalidArgument, "ingestion bindings query is invalid", problem.ValidationIssue{
			Field:   "scope",
			Message: "scope_kind and scope_key must be provided together",
		})
	}

	return nil
}

type GetActiveConfigQuery struct {
	ScopeKind string `json:"scope_kind,omitempty"`
	ScopeKey  string `json:"scope_key,omitempty"`
}

func (q GetActiveConfigQuery) Normalize() GetActiveConfigQuery {
	q.ScopeKind = strings.ToLower(strings.TrimSpace(q.ScopeKind))
	q.ScopeKey = strings.TrimSpace(q.ScopeKey)
	if q.ScopeKind == "" {
		q.ScopeKind = "global"
	}
	if q.ScopeKey == "" {
		q.ScopeKey = "default"
	}
	return q
}

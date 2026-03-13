package contracts

import (
	"strings"

	"internal/shared/problem"
)

type GetConfigQuery struct {
	ID string `json:"id"`
}

func (q GetConfigQuery) Normalize() GetConfigQuery {
	q.ID = strings.TrimSpace(q.ID)
	return q
}

func (q GetConfigQuery) Validate() *problem.Problem {
	if q.ID != "" {
		return nil
	}

	return problem.Validation(problem.InvalidArgument, "get config query is invalid", problem.ValidationIssue{
		Field:   "id",
		Message: "must not be empty",
	})
}

type ListConfigsQuery struct{}

type GetActiveConfigQuery struct{}

package contracts

import (
	"strings"
	"time"

	sharedruntime "internal/application/runtimecontracts"
)

type GetActiveRuntimeQuery struct {
	ScopeKind string `json:"scope_kind,omitempty"`
	ScopeKey  string `json:"scope_key,omitempty"`
}

func (q GetActiveRuntimeQuery) Normalize() GetActiveRuntimeQuery {
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

type GetActiveRuntimeReply struct {
	Runtime ActiveRuntimeRecord `json:"runtime"`
}

type ActiveRuntimeRecord struct {
	sharedruntime.RuntimeRecord
	LoadedAt time.Time `json:"loaded_at"`
}

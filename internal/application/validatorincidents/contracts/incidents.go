package contracts

import (
	"fmt"
	"strings"
	"time"

	sharedruntime "internal/application/runtimecontracts"
	validatorresultscontracts "internal/application/validatorresults/contracts"
	"internal/shared/problem"
)

const (
	DefaultListLimit = 20
	MaxListLimit     = 100
)

type ValidationIncidentKind string

const (
	ValidationIncidentKindRuleViolation ValidationIncidentKind = "validation.rule_violation"
)

type ValidationIncidentStatus string

const (
	ValidationIncidentStatusOpen ValidationIncidentStatus = "open"
)

type ListValidationIncidentsQuery struct {
	ScopeKind   string                   `json:"scope_kind,omitempty"`
	ScopeKey    string                   `json:"scope_key,omitempty"`
	BindingName string                   `json:"binding_name,omitempty"`
	Topic       string                   `json:"topic,omitempty"`
	Kind        ValidationIncidentKind   `json:"kind,omitempty"`
	Status      ValidationIncidentStatus `json:"status,omitempty"`
	Limit       int                      `json:"limit,omitempty"`
}

func (q ListValidationIncidentsQuery) Normalize() ListValidationIncidentsQuery {
	q.ScopeKind = strings.ToLower(strings.TrimSpace(q.ScopeKind))
	q.ScopeKey = strings.TrimSpace(q.ScopeKey)
	q.BindingName = strings.TrimSpace(q.BindingName)
	q.Topic = strings.TrimSpace(q.Topic)
	q.Kind = ValidationIncidentKind(strings.ToLower(strings.TrimSpace(string(q.Kind))))
	q.Status = ValidationIncidentStatus(strings.ToLower(strings.TrimSpace(string(q.Status))))
	if q.ScopeKind == "" {
		q.ScopeKind = "global"
	}
	if q.ScopeKey == "" {
		q.ScopeKey = "default"
	}
	switch {
	case q.Limit <= 0:
		q.Limit = DefaultListLimit
	case q.Limit > MaxListLimit:
		q.Limit = MaxListLimit
	}
	return q
}

func (q ListValidationIncidentsQuery) Validate() *problem.Problem {
	q = q.Normalize()

	var issues []problem.ValidationIssue
	if q.ScopeKind == "" {
		issues = append(issues, problem.ValidationIssue{Field: "scope_kind", Message: "must not be empty"})
	}
	if q.ScopeKey == "" {
		issues = append(issues, problem.ValidationIssue{Field: "scope_key", Message: "must not be empty"})
	}
	if q.Kind != "" && q.Kind != ValidationIncidentKindRuleViolation {
		issues = append(issues, problem.ValidationIssue{Field: "kind", Message: "must be one of validation.rule_violation", Value: q.Kind})
	}
	if q.Status != "" && q.Status != ValidationIncidentStatusOpen {
		issues = append(issues, problem.ValidationIssue{Field: "status", Message: "must be one of open", Value: q.Status})
	}
	if q.Limit <= 0 {
		issues = append(issues, problem.ValidationIssue{Field: "limit", Message: "must be greater than zero"})
	}
	if len(issues) == 0 {
		return nil
	}
	return problem.Validation(problem.InvalidArgument, "validation incidents query is invalid", issues...)
}

type ListValidationIncidentsReply struct {
	Incidents []ValidationIncidentRecord `json:"incidents"`
}

type ValidationIncidentBindingRecord struct {
	Name  string                    `json:"name"`
	Topic string                    `json:"topic"`
	Scope sharedruntime.ScopeRecord `json:"scope"`
}

type ValidationIncidentConfigRecord struct {
	SetID              string `json:"set_id"`
	Key                string `json:"key"`
	VersionID          string `json:"version_id"`
	Version            int    `json:"version"`
	DefinitionChecksum string `json:"definition_checksum"`
}

type ValidationIncidentRecord struct {
	IncidentKey         string                                      `json:"incident_key"`
	Kind                ValidationIncidentKind                      `json:"kind"`
	Status              ValidationIncidentStatus                    `json:"status"`
	Binding             ValidationIncidentBindingRecord             `json:"binding"`
	Config              ValidationIncidentConfigRecord              `json:"config"`
	Count               int                                         `json:"count"`
	FirstSeenAt         time.Time                                   `json:"first_seen_at"`
	LastSeenAt          time.Time                                   `json:"last_seen_at"`
	LatestMessageID     string                                      `json:"latest_message_id"`
	LatestCorrelationID string                                      `json:"latest_correlation_id,omitempty"`
	LatestProcessingKey string                                      `json:"latest_processing_key,omitempty"`
	Violations          []validatorresultscontracts.ViolationRecord `json:"violations,omitempty"`
}

func (r ValidationIncidentRecord) Validate() *problem.Problem {
	var issues []problem.ValidationIssue
	if strings.TrimSpace(r.IncidentKey) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "incident_key", Message: "must not be empty"})
	}
	if r.Kind != ValidationIncidentKindRuleViolation {
		issues = append(issues, problem.ValidationIssue{Field: "kind", Message: "must be validation.rule_violation", Value: r.Kind})
	}
	if r.Status != ValidationIncidentStatusOpen {
		issues = append(issues, problem.ValidationIssue{Field: "status", Message: "must be open", Value: r.Status})
	}
	if strings.TrimSpace(r.Binding.Name) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "binding.name", Message: "must not be empty"})
	}
	if strings.TrimSpace(r.Binding.Topic) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "binding.topic", Message: "must not be empty"})
	}
	if strings.TrimSpace(r.Binding.Scope.Kind) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "binding.scope.kind", Message: "must not be empty"})
	}
	if strings.TrimSpace(r.Binding.Scope.Key) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "binding.scope.key", Message: "must not be empty"})
	}
	if strings.TrimSpace(r.Config.VersionID) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "config.version_id", Message: "must not be empty"})
	}
	if r.Count <= 0 {
		issues = append(issues, problem.ValidationIssue{Field: "count", Message: "must be greater than zero"})
	}
	if r.FirstSeenAt.IsZero() {
		issues = append(issues, problem.ValidationIssue{Field: "first_seen_at", Message: "must not be zero"})
	}
	if r.LastSeenAt.IsZero() {
		issues = append(issues, problem.ValidationIssue{Field: "last_seen_at", Message: "must not be zero"})
	}
	if r.LastSeenAt.Before(r.FirstSeenAt) {
		issues = append(issues, problem.ValidationIssue{Field: "last_seen_at", Message: "must not be before first_seen_at"})
	}
	if strings.TrimSpace(r.LatestMessageID) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "latest_message_id", Message: "must not be empty"})
	}
	if len(r.Violations) == 0 {
		issues = append(issues, problem.ValidationIssue{Field: "violations", Message: "must contain at least one item"})
	}
	if len(issues) == 0 {
		return nil
	}
	return problem.Validation(problem.InvalidArgument, "validation incident is invalid", issues...)
}

func BuildIncidentKey(result validatorresultscontracts.ValidationResultRecord) string {
	return fmt.Sprintf(
		"%s|%s|%s|%s|%s|%s|%s",
		ValidationIncidentKindRuleViolation,
		strings.TrimSpace(result.Binding.Scope.Kind),
		strings.TrimSpace(result.Binding.Scope.Key),
		strings.TrimSpace(result.Binding.Name),
		strings.TrimSpace(result.Binding.Topic),
		strings.TrimSpace(result.Config.VersionID),
		validatorresultscontracts.BuildViolationFingerprint(result.Violations),
	)
}

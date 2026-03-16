package contracts

import (
	"fmt"
	"sort"
	"strings"
	"time"

	sharedruntime "internal/application/runtimecontracts"
	"internal/shared/problem"
)

const (
	DefaultListLimit = 20
	MaxListLimit     = 100
)

type ValidationStatus string

const (
	ValidationStatusPassed ValidationStatus = "passed"
	ValidationStatusFailed ValidationStatus = "failed"
)

type ListValidationResultsQuery struct {
	ScopeKind     string           `json:"scope_kind,omitempty"`
	ScopeKey      string           `json:"scope_key,omitempty"`
	BindingName   string           `json:"binding_name,omitempty"`
	Topic         string           `json:"topic,omitempty"`
	Status        ValidationStatus `json:"status,omitempty"`
	MessageID     string           `json:"message_id,omitempty"`
	CorrelationID string           `json:"correlation_id,omitempty"`
	Limit         int              `json:"limit,omitempty"`
}

func (q ListValidationResultsQuery) Normalize() ListValidationResultsQuery {
	q.ScopeKind = strings.ToLower(strings.TrimSpace(q.ScopeKind))
	q.ScopeKey = strings.TrimSpace(q.ScopeKey)
	q.BindingName = strings.TrimSpace(q.BindingName)
	q.Topic = strings.TrimSpace(q.Topic)
	q.Status = ValidationStatus(strings.ToLower(strings.TrimSpace(string(q.Status))))
	q.MessageID = strings.TrimSpace(q.MessageID)
	q.CorrelationID = strings.TrimSpace(q.CorrelationID)
	if q.ScopeKind == "" {
		q.ScopeKind = "global"
	}
	if q.ScopeKey == "" {
		q.ScopeKey = "default"
	}
	if q.Status != "" && q.Status != ValidationStatusPassed && q.Status != ValidationStatusFailed {
		q.Status = ValidationStatus(strings.TrimSpace(string(q.Status)))
	}
	switch {
	case q.Limit <= 0:
		q.Limit = DefaultListLimit
	case q.Limit > MaxListLimit:
		q.Limit = MaxListLimit
	}
	return q
}

func (q ListValidationResultsQuery) Validate() *problem.Problem {
	q = q.Normalize()

	var issues []problem.ValidationIssue
	if q.ScopeKind == "" {
		issues = append(issues, problem.ValidationIssue{Field: "scope_kind", Message: "must not be empty"})
	}
	if q.ScopeKey == "" {
		issues = append(issues, problem.ValidationIssue{Field: "scope_key", Message: "must not be empty"})
	}
	if q.Status != "" && q.Status != ValidationStatusPassed && q.Status != ValidationStatusFailed {
		issues = append(issues, problem.ValidationIssue{Field: "status", Message: "must be one of passed or failed", Value: q.Status})
	}
	if q.Limit <= 0 {
		issues = append(issues, problem.ValidationIssue{Field: "limit", Message: "must be greater than zero"})
	}
	if len(issues) == 0 {
		return nil
	}
	return problem.Validation(problem.InvalidArgument, "validation results query is invalid", issues...)
}

type ListValidationResultsReply struct {
	Results []ValidationResultRecord `json:"results"`
}

type ValidationBindingRecord struct {
	Name  string                    `json:"name"`
	Topic string                    `json:"topic"`
	Scope sharedruntime.ScopeRecord `json:"scope"`
}

type ValidationConfigRecord struct {
	SetID              string `json:"set_id"`
	Key                string `json:"key"`
	VersionID          string `json:"version_id"`
	Version            int    `json:"version"`
	DefinitionChecksum string `json:"definition_checksum"`
}

type ViolationRecord struct {
	Rule     string `json:"rule"`
	Field    string `json:"field"`
	Operator string `json:"operator"`
	Severity string `json:"severity"`
	Message  string `json:"message"`
}

type ValidationResultRecord struct {
	ProcessingKey string                  `json:"processing_key,omitempty"`
	MessageID     string                  `json:"message_id"`
	CorrelationID string                  `json:"correlation_id,omitempty"`
	Binding       ValidationBindingRecord `json:"binding"`
	Config        ValidationConfigRecord  `json:"config"`
	Status        ValidationStatus        `json:"status"`
	Violations    []ViolationRecord       `json:"violations,omitempty"`
	ProcessedAt   time.Time               `json:"processed_at"`
}

func (r ValidationResultRecord) Validate() *problem.Problem {
	var issues []problem.ValidationIssue
	if strings.TrimSpace(r.NormalizedProcessingKey()) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "processing_key", Message: "must not be empty"})
	}
	if strings.TrimSpace(r.MessageID) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "message_id", Message: "must not be empty"})
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
	switch r.Status {
	case ValidationStatusPassed:
		if len(r.Violations) > 0 {
			issues = append(issues, problem.ValidationIssue{Field: "violations", Message: "must be empty when status is passed"})
		}
	case ValidationStatusFailed:
		if len(r.Violations) == 0 {
			issues = append(issues, problem.ValidationIssue{Field: "violations", Message: "must contain at least one item when status is failed"})
		}
	default:
		issues = append(issues, problem.ValidationIssue{Field: "status", Message: "must be one of passed or failed", Value: r.Status})
	}
	if r.ProcessedAt.IsZero() {
		issues = append(issues, problem.ValidationIssue{Field: "processed_at", Message: "must not be zero"})
	}
	if len(issues) == 0 {
		return nil
	}
	return problem.Validation(problem.InvalidArgument, "validation result is invalid", issues...)
}

func (r ValidationResultRecord) NormalizedProcessingKey() string {
	if key := strings.TrimSpace(r.ProcessingKey); key != "" {
		return key
	}
	return BuildValidationProcessingKey(r.MessageID, r.Binding, r.Config)
}

func BuildValidationProcessingKey(messageID string, binding ValidationBindingRecord, config ValidationConfigRecord) string {
	messageID = strings.TrimSpace(messageID)
	scopeKind := strings.TrimSpace(binding.Scope.Kind)
	scopeKey := strings.TrimSpace(binding.Scope.Key)
	bindingName := strings.TrimSpace(binding.Name)
	bindingTopic := strings.TrimSpace(binding.Topic)
	configVersionID := strings.TrimSpace(config.VersionID)
	configChecksum := strings.TrimSpace(config.DefinitionChecksum)

	parts := []string{
		messageID,
		scopeKind,
		scopeKey,
		bindingName,
		bindingTopic,
		configVersionID,
		configChecksum,
	}
	for _, part := range parts {
		if part != "" {
			return strings.Join(parts, "|")
		}
	}
	return ""
}

func BuildViolationFingerprint(violations []ViolationRecord) string {
	if len(violations) == 0 {
		return ""
	}

	fingerprints := make([]string, 0, len(violations))
	for _, violation := range violations {
		fingerprints = append(fingerprints, fmt.Sprintf(
			"%s:%s:%s:%s",
			strings.TrimSpace(violation.Rule),
			strings.TrimSpace(violation.Field),
			strings.TrimSpace(violation.Operator),
			strings.TrimSpace(violation.Severity),
		))
	}
	sort.Strings(fingerprints)
	return strings.Join(fingerprints, ",")
}

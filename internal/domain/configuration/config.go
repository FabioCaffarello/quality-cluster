package configuration

import (
	"encoding/json"
	"strings"
	"time"

	"internal/shared/problem"

	"gopkg.in/yaml.v3"
)

type Format string

const (
	FormatJSON Format = "json"
	FormatYAML Format = "yaml"
	FormatText Format = "text"
)

type Status string

const (
	StatusDraft  Status = "draft"
	StatusActive Status = "active"
)

type Config struct {
	ID        string
	Name      string
	Format    Format
	Content   string
	Status    Status
	CreatedAt time.Time
	UpdatedAt time.Time
}

type ValidationDiagnostic struct {
	Field   string
	Message string
}

func NewDraft(id, name string, format Format, content string, createdAt time.Time) (Config, *problem.Problem) {
	name = strings.TrimSpace(name)
	content = strings.TrimSpace(content)

	var issues []problem.ValidationIssue

	if strings.TrimSpace(id) == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "id",
			Message: "must not be empty",
		})
	}

	if name == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "name",
			Message: "must not be empty",
		})
	}

	switch format {
	case FormatJSON, FormatYAML, FormatText:
	default:
		issues = append(issues, problem.ValidationIssue{
			Field:   "format",
			Message: "must be one of json, yaml or text",
			Value:   format,
		})
	}

	if content == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "content",
			Message: "must not be empty",
		})
	}

	if diagnostics, prob := ValidateContent(format, content); prob != nil {
		return Config{}, prob
	} else if len(diagnostics) > 0 {
		for _, diagnostic := range diagnostics {
			issues = append(issues, problem.ValidationIssue{
				Field:   diagnostic.Field,
				Message: diagnostic.Message,
			})
		}
	}

	if createdAt.IsZero() {
		issues = append(issues, problem.ValidationIssue{
			Field:   "created_at",
			Message: "must not be zero",
		})
	}

	if len(issues) > 0 {
		return Config{}, problem.Validation(problem.InvalidArgument, "configuration is invalid", issues...)
	}

	return Config{
		ID:        strings.TrimSpace(id),
		Name:      name,
		Format:    format,
		Content:   content,
		Status:    StatusDraft,
		CreatedAt: createdAt.UTC(),
		UpdatedAt: createdAt.UTC(),
	}, nil
}

func ValidateContent(format Format, content string) ([]ValidationDiagnostic, *problem.Problem) {
	content = strings.TrimSpace(content)
	if content == "" {
		return []ValidationDiagnostic{{
			Field:   "content",
			Message: "must not be empty",
		}}, nil
	}

	switch format {
	case FormatJSON:
		var payload any
		if err := json.Unmarshal([]byte(content), &payload); err != nil {
			return []ValidationDiagnostic{{
				Field:   "content",
				Message: "must be valid JSON",
			}}, nil
		}
	case FormatYAML:
		var payload any
		if err := yaml.Unmarshal([]byte(content), &payload); err != nil {
			return []ValidationDiagnostic{{
				Field:   "content",
				Message: "must be valid YAML",
			}}, nil
		}
	case FormatText:
		return nil, nil
	default:
		return nil, problem.Validation(problem.InvalidArgument, "configuration content is invalid", problem.ValidationIssue{
			Field:   "format",
			Message: "must be one of json, yaml or text",
			Value:   format,
		})
	}

	return nil, nil
}

package configctl

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"strconv"
	"strings"

	"internal/shared/problem"

	"gopkg.in/yaml.v3"
)

type SourceFormat string

const (
	FormatJSON SourceFormat = "json"
	FormatYAML SourceFormat = "yaml"
)

type ConfigSource struct {
	Format  SourceFormat
	Content string
}

type ValidationDiagnostic struct {
	Field   string
	Message string
}

type ConfigDocument struct {
	Metadata ConfigMetadata `json:"metadata" yaml:"metadata"`
	Bindings []Binding      `json:"bindings" yaml:"bindings"`
	Fields   []Field        `json:"fields" yaml:"fields"`
	Rules    []Rule         `json:"rules" yaml:"rules"`
}

type ConfigMetadata struct {
	Name        string            `json:"name" yaml:"name"`
	Description string            `json:"description,omitempty" yaml:"description,omitempty"`
	Labels      map[string]string `json:"labels,omitempty" yaml:"labels,omitempty"`
}

type Binding struct {
	Name  string `json:"name" yaml:"name"`
	Topic string `json:"topic" yaml:"topic"`
}

type FieldType string

const (
	FieldTypeString    FieldType = "string"
	FieldTypeInteger   FieldType = "integer"
	FieldTypeNumber    FieldType = "number"
	FieldTypeBoolean   FieldType = "boolean"
	FieldTypeTimestamp FieldType = "timestamp"
)

type Field struct {
	Name     string    `json:"name" yaml:"name"`
	Type     FieldType `json:"type" yaml:"type"`
	Required bool      `json:"required,omitempty" yaml:"required,omitempty"`
}

type RuleOperator string

const (
	RuleOperatorRequired RuleOperator = "required"
	RuleOperatorNotEmpty RuleOperator = "not_empty"
	RuleOperatorEquals   RuleOperator = "equals"
)

type RuleSeverity string

const (
	RuleSeverityError RuleSeverity = "error"
	RuleSeverityWarn  RuleSeverity = "warn"
)

type Rule struct {
	Name          string       `json:"name" yaml:"name"`
	Field         string       `json:"field" yaml:"field"`
	Operator      RuleOperator `json:"operator" yaml:"operator"`
	ExpectedValue string       `json:"expected_value,omitempty" yaml:"expected_value,omitempty"`
	Severity      RuleSeverity `json:"severity,omitempty" yaml:"severity,omitempty"`
}

func (s ConfigSource) Normalize() ConfigSource {
	s.Format = SourceFormat(strings.ToLower(strings.TrimSpace(string(s.Format))))
	s.Content = strings.TrimSpace(s.Content)
	return s
}

func (s ConfigSource) ValidateForDraft() *problem.Problem {
	s = s.Normalize()

	var issues []problem.ValidationIssue
	if s.Content == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "content",
			Message: "must not be empty",
		})
	}

	switch s.Format {
	case FormatJSON, FormatYAML:
	default:
		issues = append(issues, problem.ValidationIssue{
			Field:   "format",
			Message: "must be one of json or yaml",
			Value:   s.Format,
		})
	}

	if len(issues) == 0 {
		return nil
	}

	return problem.Validation(problem.InvalidArgument, "config source is invalid", issues...)
}

func (s ConfigSource) Checksum() string {
	s = s.Normalize()
	return checksum(string(s.Format) + ":" + s.Content)
}

func InspectDocument(source ConfigSource) (ConfigDocument, []ValidationDiagnostic, *problem.Problem) {
	source = source.Normalize()
	if prob := source.ValidateForDraft(); prob != nil {
		return ConfigDocument{}, nil, prob
	}

	var doc ConfigDocument
	switch source.Format {
	case FormatJSON:
		if err := json.Unmarshal([]byte(source.Content), &doc); err != nil {
			return ConfigDocument{}, []ValidationDiagnostic{{
				Field:   "content",
				Message: "must be valid JSON",
			}}, nil
		}
	case FormatYAML:
		if err := yaml.Unmarshal([]byte(source.Content), &doc); err != nil {
			return ConfigDocument{}, []ValidationDiagnostic{{
				Field:   "content",
				Message: "must be valid YAML",
			}}, nil
		}
	default:
		return ConfigDocument{}, nil, problem.Validation(problem.InvalidArgument, "config source is invalid", problem.ValidationIssue{
			Field:   "format",
			Message: "must be one of json or yaml",
			Value:   source.Format,
		})
	}

	diagnostics := doc.Validate()
	if len(diagnostics) > 0 {
		return ConfigDocument{}, diagnostics, nil
	}

	return doc.normalize(), nil, nil
}

func (d ConfigDocument) Validate() []ValidationDiagnostic {
	normalized := d.normalize()
	var diagnostics []ValidationDiagnostic

	if normalized.Metadata.Name == "" {
		diagnostics = append(diagnostics, ValidationDiagnostic{
			Field:   "metadata.name",
			Message: "must not be empty",
		})
	}

	if len(normalized.Bindings) == 0 {
		diagnostics = append(diagnostics, ValidationDiagnostic{
			Field:   "bindings",
			Message: "must contain at least one binding",
		})
	}

	if len(normalized.Fields) == 0 {
		diagnostics = append(diagnostics, ValidationDiagnostic{
			Field:   "fields",
			Message: "must contain at least one field",
		})
	}

	if len(normalized.Rules) == 0 {
		diagnostics = append(diagnostics, ValidationDiagnostic{
			Field:   "rules",
			Message: "must contain at least one rule",
		})
	}

	bindingNames := make(map[string]struct{}, len(normalized.Bindings))
	bindingTopics := make(map[string]struct{}, len(normalized.Bindings))
	for index, binding := range normalized.Bindings {
		fieldPath := "bindings[" + itoa(index) + "]"
		if binding.Name == "" {
			diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".name", Message: "must not be empty"})
		}
		if binding.Topic == "" {
			diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".topic", Message: "must not be empty"})
		}
		if binding.Name != "" {
			if _, exists := bindingNames[binding.Name]; exists {
				diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".name", Message: "must be unique"})
			}
			bindingNames[binding.Name] = struct{}{}
		}
		if binding.Topic != "" {
			if _, exists := bindingTopics[binding.Topic]; exists {
				diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".topic", Message: "must be unique"})
			}
			bindingTopics[binding.Topic] = struct{}{}
		}
	}

	fieldNames := make(map[string]struct{}, len(normalized.Fields))
	for index, field := range normalized.Fields {
		fieldPath := "fields[" + itoa(index) + "]"
		if field.Name == "" {
			diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".name", Message: "must not be empty"})
		}
		switch field.Type {
		case FieldTypeString, FieldTypeInteger, FieldTypeNumber, FieldTypeBoolean, FieldTypeTimestamp:
		default:
			diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".type", Message: "must be one of string, integer, number, boolean or timestamp"})
		}
		if field.Name != "" {
			if _, exists := fieldNames[field.Name]; exists {
				diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".name", Message: "must be unique"})
			}
			fieldNames[field.Name] = struct{}{}
		}
	}

	ruleNames := make(map[string]struct{}, len(normalized.Rules))
	for index, rule := range normalized.Rules {
		fieldPath := "rules[" + itoa(index) + "]"
		if rule.Name == "" {
			diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".name", Message: "must not be empty"})
		}
		if rule.Field == "" {
			diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".field", Message: "must not be empty"})
		} else if _, exists := fieldNames[rule.Field]; !exists {
			diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".field", Message: "must reference an existing field"})
		}
		switch rule.Operator {
		case RuleOperatorRequired, RuleOperatorNotEmpty, RuleOperatorEquals:
		default:
			diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".operator", Message: "must be one of required, not_empty or equals"})
		}
		switch rule.Severity {
		case "", RuleSeverityError, RuleSeverityWarn:
		default:
			diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".severity", Message: "must be one of error or warn"})
		}
		if rule.Operator == RuleOperatorEquals && rule.ExpectedValue == "" {
			diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".expected_value", Message: "must not be empty when operator is equals"})
		}
		if rule.Name != "" {
			if _, exists := ruleNames[rule.Name]; exists {
				diagnostics = append(diagnostics, ValidationDiagnostic{Field: fieldPath + ".name", Message: "must be unique"})
			}
			ruleNames[rule.Name] = struct{}{}
		}
	}

	return diagnostics
}

func (d ConfigDocument) Checksum() string {
	normalized := d.normalize()
	payload, err := json.Marshal(normalized)
	if err != nil {
		return checksum(normalized.Metadata.Name)
	}
	return checksum(string(payload))
}

func (d ConfigDocument) normalize() ConfigDocument {
	normalized := ConfigDocument{
		Metadata: ConfigMetadata{
			Name:        strings.TrimSpace(d.Metadata.Name),
			Description: strings.TrimSpace(d.Metadata.Description),
		},
		Bindings: make([]Binding, 0, len(d.Bindings)),
		Fields:   make([]Field, 0, len(d.Fields)),
		Rules:    make([]Rule, 0, len(d.Rules)),
	}

	if len(d.Metadata.Labels) > 0 {
		normalized.Metadata.Labels = make(map[string]string, len(d.Metadata.Labels))
		for key, value := range d.Metadata.Labels {
			key = strings.TrimSpace(key)
			value = strings.TrimSpace(value)
			if key == "" || value == "" {
				continue
			}
			normalized.Metadata.Labels[key] = value
		}
	}

	for _, binding := range d.Bindings {
		normalized.Bindings = append(normalized.Bindings, Binding{
			Name:  strings.TrimSpace(binding.Name),
			Topic: strings.TrimSpace(binding.Topic),
		})
	}

	for _, field := range d.Fields {
		normalized.Fields = append(normalized.Fields, Field{
			Name:     strings.TrimSpace(field.Name),
			Type:     FieldType(strings.ToLower(strings.TrimSpace(string(field.Type)))),
			Required: field.Required,
		})
	}

	for _, rule := range d.Rules {
		normalized.Rules = append(normalized.Rules, Rule{
			Name:          strings.TrimSpace(rule.Name),
			Field:         strings.TrimSpace(rule.Field),
			Operator:      RuleOperator(strings.ToLower(strings.TrimSpace(string(rule.Operator)))),
			ExpectedValue: strings.TrimSpace(rule.ExpectedValue),
			Severity:      RuleSeverity(strings.ToLower(strings.TrimSpace(string(rule.Severity)))),
		})
	}

	return normalized
}

func checksum(value string) string {
	sum := sha256.Sum256([]byte(value))
	return hex.EncodeToString(sum[:])
}

func itoa(value int) string {
	return strconv.Itoa(value)
}

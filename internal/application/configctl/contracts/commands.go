package contracts

import (
	"strings"

	"internal/shared/problem"
)

type CreateDraftCommand struct {
	Name    string `json:"name"`
	Format  string `json:"format"`
	Content string `json:"content"`
}

func (c CreateDraftCommand) Normalize() CreateDraftCommand {
	c.Name = strings.TrimSpace(c.Name)
	c.Format = strings.ToLower(strings.TrimSpace(c.Format))
	c.Content = strings.TrimSpace(c.Content)
	return c
}

func (c CreateDraftCommand) Validate() *problem.Problem {
	return validateConfigInput("create draft command", c.Name, c.Format, c.Content, true)
}

type ValidateDraftCommand struct {
	Format  string `json:"format"`
	Content string `json:"content"`
}

func (c ValidateDraftCommand) Normalize() ValidateDraftCommand {
	c.Format = strings.ToLower(strings.TrimSpace(c.Format))
	c.Content = strings.TrimSpace(c.Content)
	return c
}

func (c ValidateDraftCommand) Validate() *problem.Problem {
	return validateConfigInput("validate draft command", "", c.Format, c.Content, false)
}

func validateConfigInput(message, name, format, content string, requireName bool) *problem.Problem {
	var issues []problem.ValidationIssue

	if requireName && name == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "name",
			Message: "must not be empty",
		})
	}

	if format == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "format",
			Message: "must not be empty",
		})
	}

	switch format {
	case "json", "yaml", "text":
	case "":
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

	if len(issues) == 0 {
		return nil
	}

	return problem.Validation(problem.InvalidArgument, message+" is invalid", issues...)
}

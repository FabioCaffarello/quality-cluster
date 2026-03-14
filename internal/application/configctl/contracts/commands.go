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

type ValidateConfigCommand struct {
	VersionID string `json:"version_id"`
}

func (c ValidateConfigCommand) Normalize() ValidateConfigCommand {
	c.VersionID = strings.TrimSpace(c.VersionID)
	return c
}

func (c ValidateConfigCommand) Validate() *problem.Problem {
	return validateVersionID("validate config command", c.VersionID)
}

type CompileConfigCommand struct {
	VersionID       string `json:"version_id"`
	ArtifactID      string `json:"artifact_id"`
	SchemaVersion   string `json:"schema_version"`
	Checksum        string `json:"checksum"`
	StorageRef      string `json:"storage_ref"`
	RuntimeLoader   string `json:"runtime_loader"`
	CompilerVersion string `json:"compiler_version"`
}

func (c CompileConfigCommand) Normalize() CompileConfigCommand {
	c.VersionID = strings.TrimSpace(c.VersionID)
	c.ArtifactID = strings.TrimSpace(c.ArtifactID)
	c.SchemaVersion = strings.TrimSpace(c.SchemaVersion)
	c.Checksum = strings.TrimSpace(c.Checksum)
	c.StorageRef = strings.TrimSpace(c.StorageRef)
	c.RuntimeLoader = strings.TrimSpace(c.RuntimeLoader)
	c.CompilerVersion = strings.TrimSpace(c.CompilerVersion)
	return c
}

func (c CompileConfigCommand) Validate() *problem.Problem {
	return validateVersionID("compile config command", c.Normalize().VersionID)
}

type ActivateConfigCommand struct {
	VersionID string `json:"version_id"`
	ScopeKind string `json:"scope_kind"`
	ScopeKey  string `json:"scope_key"`
}

func (c ActivateConfigCommand) Normalize() ActivateConfigCommand {
	c.VersionID = strings.TrimSpace(c.VersionID)
	c.ScopeKind = strings.ToLower(strings.TrimSpace(c.ScopeKind))
	c.ScopeKey = strings.TrimSpace(c.ScopeKey)
	return c
}

func (c ActivateConfigCommand) Validate() *problem.Problem {
	c = c.Normalize()
	var issues []problem.ValidationIssue
	if prob := validateVersionID("activate config command", c.VersionID); prob != nil {
		issues = append(issues, validationIssues(prob)...)
	}
	if c.ScopeKind == "" {
		c.ScopeKind = "global"
	}
	if c.ScopeKey == "" {
		c.ScopeKey = "default"
	}
	if len(issues) == 0 {
		return nil
	}
	return problem.Validation(problem.InvalidArgument, "activate config command is invalid", issues...)
}

type DeactivateConfigCommand struct {
	ScopeKind string `json:"scope_kind"`
	ScopeKey  string `json:"scope_key"`
}

func (c DeactivateConfigCommand) Normalize() DeactivateConfigCommand {
	c.ScopeKind = strings.ToLower(strings.TrimSpace(c.ScopeKind))
	c.ScopeKey = strings.TrimSpace(c.ScopeKey)
	return c
}

func (c DeactivateConfigCommand) Validate() *problem.Problem {
	return nil
}

type ArchiveConfigCommand struct {
	VersionID string `json:"version_id"`
}

func (c ArchiveConfigCommand) Normalize() ArchiveConfigCommand {
	c.VersionID = strings.TrimSpace(c.VersionID)
	return c
}

func (c ArchiveConfigCommand) Validate() *problem.Problem {
	return validateVersionID("archive config command", c.VersionID)
}

type RejectConfigCommand struct {
	VersionID string `json:"version_id"`
	Reason    string `json:"reason"`
}

func (c RejectConfigCommand) Normalize() RejectConfigCommand {
	c.VersionID = strings.TrimSpace(c.VersionID)
	c.Reason = strings.TrimSpace(c.Reason)
	return c
}

func (c RejectConfigCommand) Validate() *problem.Problem {
	return validateVersionID("reject config command", c.VersionID)
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
	case "json", "yaml":
	case "":
	default:
		issues = append(issues, problem.ValidationIssue{
			Field:   "format",
			Message: "must be one of json or yaml",
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

func validateVersionID(message, versionID string) *problem.Problem {
	if strings.TrimSpace(versionID) != "" {
		return nil
	}
	return problem.Validation(problem.InvalidArgument, message+" is invalid", problem.ValidationIssue{
		Field:   "version_id",
		Message: "must not be empty",
	})
}

func validationIssues(prob *problem.Problem) []problem.ValidationIssue {
	if prob == nil || prob.Details == nil {
		return nil
	}
	raw, ok := prob.Details[problem.DetailIssues]
	if !ok {
		return nil
	}
	issues, ok := raw.([]problem.ValidationIssue)
	if !ok {
		return nil
	}
	return issues
}

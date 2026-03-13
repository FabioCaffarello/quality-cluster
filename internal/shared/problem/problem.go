package problem

import (
	"errors"
	"fmt"
)

type ProblemCode string

const (
	// ValidationFailed indicates generic input validation failure.
	ValidationFailed ProblemCode = "VAL_VALIDATION_FAILED"
	// InvalidArgument indicates invalid argument shape/value.
	InvalidArgument ProblemCode = "VAL_INVALID_ARGUMENT"

	// NotFound indicates requested resource was not found.
	NotFound ProblemCode = "SYS_NOT_FOUND"
	// Conflict indicates conflict with current system state.
	Conflict ProblemCode = "SYS_CONFLICT"
	// Internal indicates unexpected system/internal error.
	Internal ProblemCode = "SYS_INTERNAL"
	// Unavailable indicates temporary system/network unavailability.
	Unavailable ProblemCode = "SYS_UNAVAILABLE"
)

const (
	DetailField  = "field"
	DetailValue  = "value"
	DetailIssues = "issues"
)

// ValidationIssue describes a single invalid field or invariant.
type ValidationIssue struct {
	Field   string `json:"field,omitempty"`
	Message string `json:"message"`
	Value   any    `json:"value,omitempty"`
}

// Problem is the canonical error type for the system.
// It carries a stable code, a human-readable message and optional structured details.
type Problem struct {
	Code      ProblemCode    `json:"code"`
	Message   string         `json:"message"`
	Details   map[string]any `json:"details,omitempty"`
	Retryable bool           `json:"retryable,omitempty"`
	Cause     error          `json:"-"`
}

// Error implements the error interface so Problem can interoperate with stdlib.
func (p *Problem) Error() string {
	if p == nil {
		return "<nil>"
	}
	if p.Cause != nil {
		return fmt.Sprintf("%s: %s: %v", p.Code, p.Message, p.Cause)
	}
	return fmt.Sprintf("%s: %s", p.Code, p.Message)
}

// Unwrap allows errors.Is / errors.As traversal.
func (p *Problem) Unwrap() error {
	if p == nil {
		return nil
	}
	return p.Cause
}

// Clone returns a deep copy of the problem.
func (p *Problem) Clone() *Problem {
	if p == nil {
		return nil
	}

	clone := *p
	if len(p.Details) > 0 {
		clone.Details = make(map[string]any, len(p.Details))
		for key, value := range p.Details {
			clone.Details[key] = value
		}
	}

	return &clone
}

// WithDetail returns a copy of p with one detail entry added or replaced.
func (p *Problem) WithDetail(key string, value any) *Problem {
	if p == nil {
		return nil
	}

	clone := p.Clone()
	if clone.Details == nil {
		clone.Details = make(map[string]any, 1)
	}
	clone.Details[key] = value

	return clone
}

// WithDetails returns a copy of p with all detail entries merged.
func (p *Problem) WithDetails(details map[string]any) *Problem {
	if p == nil || len(details) == 0 {
		return p.Clone()
	}

	clone := p.Clone()
	if clone.Details == nil {
		clone.Details = make(map[string]any, len(details))
	}
	for key, value := range details {
		clone.Details[key] = value
	}

	return clone
}

// WithCause returns a copy of p with a new underlying cause.
func (p *Problem) WithCause(err error) *Problem {
	if p == nil {
		return nil
	}

	clone := p.Clone()
	clone.Cause = err
	return clone
}

// MarkRetryable returns a copy of p marked as retryable.
func (p *Problem) MarkRetryable() *Problem {
	if p == nil {
		return nil
	}

	clone := p.Clone()
	clone.Retryable = true
	return clone
}

// New creates a Problem with code and message.
func New(code ProblemCode, msg string) *Problem {
	return &Problem{
		Code:    code,
		Message: msg,
	}
}

// Newf creates a Problem with a formatted message.
func Newf(code ProblemCode, format string, args ...any) *Problem {
	return New(code, fmt.Sprintf(format, args...))
}

// Validation builds a validation problem with structured issues.
func Validation(code ProblemCode, message string, issues ...ValidationIssue) *Problem {
	prob := New(code, message)
	if len(issues) == 0 {
		return prob
	}

	copied := make([]ValidationIssue, len(issues))
	copy(copied, issues)
	prob.Details = map[string]any{
		DetailIssues: copied,
	}
	return prob
}

// Wrap wraps an existing error as the cause of a new Problem.
func Wrap(err error, code ProblemCode, msg string) *Problem {
	return New(code, msg).WithCause(err)
}

// Wrapf wraps an existing error with a formatted message.
func Wrapf(err error, code ProblemCode, format string, args ...any) *Problem {
	return Wrap(err, code, fmt.Sprintf(format, args...))
}

// From normalizes any error into the canonical Problem type.
func From(err error) *Problem {
	if err == nil {
		return nil
	}

	var prob *Problem
	if errors.As(err, &prob) {
		return prob
	}

	return Wrap(err, Internal, "unexpected error")
}

// IsCode reports whether err or any wrapped error is a Problem with the given code.
func IsCode(err error, code ProblemCode) bool {
	var prob *Problem
	if !errors.As(err, &prob) {
		return false
	}
	return prob != nil && prob.Code == code
}

// WithDetail preserves the original helper-style API for call sites that prefer functions.
func WithDetail(p *Problem, key string, value any) *Problem {
	if p == nil {
		return nil
	}
	return p.WithDetail(key, value)
}

// WithRetryable preserves the original helper-style API for call sites that prefer functions.
func WithRetryable(p *Problem) *Problem {
	if p == nil {
		return nil
	}
	return p.MarkRetryable()
}

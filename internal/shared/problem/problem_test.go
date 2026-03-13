package problem

import (
	"errors"
	"testing"
)

func TestCloneAndDetailsAreImmutable(t *testing.T) {
	base := New(InvalidArgument, "invalid request").WithDetail("field", "log.level")

	derived := base.WithDetail("value", "verbose").MarkRetryable()

	if _, exists := base.Details["value"]; exists {
		t.Fatalf("base problem was mutated")
	}
	if !derived.Retryable {
		t.Fatalf("derived problem should be retryable")
	}
}

func TestValidationCarriesIssues(t *testing.T) {
	prob := Validation(ValidationFailed, "config invalid", ValidationIssue{
		Field:   "http.addr",
		Message: "must not be empty",
	})

	rawIssues, ok := prob.Details[DetailIssues]
	if !ok {
		t.Fatalf("expected validation issues in problem details")
	}

	issues, ok := rawIssues.([]ValidationIssue)
	if !ok {
		t.Fatalf("expected validation issues to be strongly typed")
	}

	if len(issues) != 1 || issues[0].Field != "http.addr" {
		t.Fatalf("unexpected issues payload: %#v", issues)
	}
}

func TestFromAndIsCode(t *testing.T) {
	root := errors.New("boom")
	wrapped := Wrap(root, Unavailable, "dependency unavailable")

	if !errors.Is(wrapped, root) {
		t.Fatalf("wrapped problem should unwrap original error")
	}

	if !IsCode(wrapped, Unavailable) {
		t.Fatalf("expected IsCode to match wrapped problem")
	}

	if got := From(root); got.Code != Internal {
		t.Fatalf("expected non-problem errors to normalize to internal, got %q", got.Code)
	}
}

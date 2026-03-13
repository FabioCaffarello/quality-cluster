package events

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"time"

	"internal/shared/problem"
)

type Name string

// Event is the broker-agnostic application event contract shared across layers.
type Event interface {
	EventName() Name
	EventMetadata() Metadata
}

// Metadata carries stable event metadata independent of transport concerns.
type Metadata struct {
	ID            string    `json:"id"`
	OccurredAt    time.Time `json:"occurred_at"`
	CorrelationID string    `json:"correlation_id,omitempty"`
	CausationID   string    `json:"causation_id,omitempty"`
}

func NewMetadata() Metadata {
	return Metadata{
		ID:         newID(),
		OccurredAt: time.Now().UTC(),
	}
}

func (m Metadata) WithCorrelationID(correlationID string) Metadata {
	m.CorrelationID = correlationID
	return m
}

func (m Metadata) WithCausationID(causationID string) Metadata {
	m.CausationID = causationID
	return m
}

func (m Metadata) WithOccurredAt(occurredAt time.Time) Metadata {
	m.OccurredAt = occurredAt.UTC()
	return m
}

func (m Metadata) Validate() *problem.Problem {
	var issues []problem.ValidationIssue

	if m.ID == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "id",
			Message: "must not be empty",
		})
	}

	if m.OccurredAt.IsZero() {
		issues = append(issues, problem.ValidationIssue{
			Field:   "occurred_at",
			Message: "must not be zero",
		})
	}

	if len(issues) == 0 {
		return nil
	}

	return problem.Validation(problem.InvalidArgument, "event metadata is invalid", issues...)
}

func Validate(event Event) *problem.Problem {
	if event == nil {
		return problem.New(problem.InvalidArgument, "event is required")
	}

	if event.EventName() == "" {
		return problem.Validation(problem.InvalidArgument, "event is invalid", problem.ValidationIssue{
			Field:   "name",
			Message: "must not be empty",
		})
	}

	return event.EventMetadata().Validate()
}

type Handler interface {
	Handle(context.Context, Event) error
}

type HandlerFunc func(context.Context, Event) error

func (fn HandlerFunc) Handle(ctx context.Context, event Event) error {
	return fn(ctx, event)
}

func newID() string {
	var raw [16]byte
	if _, err := rand.Read(raw[:]); err != nil {
		return time.Now().UTC().Format("20060102150405.000000000")
	}
	return hex.EncodeToString(raw[:])
}

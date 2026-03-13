package envelope

import (
	"crypto/rand"
	"encoding/hex"
	"time"

	"internal/shared/problem"
)

type Kind string

const (
	KindCommand Kind = "command"
	KindEvent   Kind = "event"
	KindRequest Kind = "request"
	KindReply   Kind = "reply"
)

const DefaultContentType = "application/json"

// Envelope is the shared transport contract for internal asynchronous and request/reply messages.
type Envelope[T any] struct {
	ID            string            `json:"id"`
	Kind          Kind              `json:"kind"`
	Type          string            `json:"type"`
	Source        string            `json:"source,omitempty"`
	Subject       string            `json:"subject,omitempty"`
	CorrelationID string            `json:"correlation_id,omitempty"`
	CausationID   string            `json:"causation_id,omitempty"`
	ReplyTo       string            `json:"reply_to,omitempty"`
	ContentType   string            `json:"content_type,omitempty"`
	Timestamp     time.Time         `json:"timestamp"`
	Headers       map[string]string `json:"headers,omitempty"`
	Payload       T                 `json:"payload,omitempty"`
	Problem       *problem.Problem  `json:"problem,omitempty"`
}

// New creates a new envelope with stable defaults for internal messaging.
func New[T any](kind Kind, messageType string, payload T) Envelope[T] {
	return Envelope[T]{
		ID:          newID(),
		Kind:        kind,
		Type:        messageType,
		ContentType: DefaultContentType,
		Timestamp:   time.Now().UTC(),
		Payload:     payload,
	}
}

func (e Envelope[T]) WithSource(source string) Envelope[T] {
	e.Source = source
	return e
}

func (e Envelope[T]) WithID(id string) Envelope[T] {
	e.ID = id
	return e
}

func (e Envelope[T]) WithSubject(subject string) Envelope[T] {
	e.Subject = subject
	return e
}

func (e Envelope[T]) WithCorrelationID(correlationID string) Envelope[T] {
	e.CorrelationID = correlationID
	return e
}

func (e Envelope[T]) WithCausationID(causationID string) Envelope[T] {
	e.CausationID = causationID
	return e
}

func (e Envelope[T]) WithReplyTo(replyTo string) Envelope[T] {
	e.ReplyTo = replyTo
	return e
}

func (e Envelope[T]) WithContentType(contentType string) Envelope[T] {
	e.ContentType = contentType
	return e
}

func (e Envelope[T]) WithTimestamp(timestamp time.Time) Envelope[T] {
	e.Timestamp = timestamp
	return e
}

func (e Envelope[T]) WithProblem(prob *problem.Problem) Envelope[T] {
	e.Problem = prob
	return e
}

func (e Envelope[T]) WithHeader(key, value string) Envelope[T] {
	if e.Headers == nil {
		e.Headers = make(map[string]string, 1)
	}
	e.Headers[key] = value
	return e
}

// Validate checks the envelope metadata invariants shared across transports.
func (e Envelope[T]) Validate() *problem.Problem {
	var issues []problem.ValidationIssue

	switch e.Kind {
	case KindCommand, KindEvent, KindRequest, KindReply:
	default:
		issues = append(issues, problem.ValidationIssue{
			Field:   "kind",
			Message: "must be one of command, event, request or reply",
			Value:   e.Kind,
		})
	}

	if e.ID == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "id",
			Message: "must not be empty",
		})
	}
	if e.Type == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "type",
			Message: "must not be empty",
		})
	}
	if e.Timestamp.IsZero() {
		issues = append(issues, problem.ValidationIssue{
			Field:   "timestamp",
			Message: "must not be zero",
		})
	}
	if e.ContentType == "" {
		issues = append(issues, problem.ValidationIssue{
			Field:   "content_type",
			Message: "must not be empty",
		})
	}

	for key := range e.Headers {
		if key == "" {
			issues = append(issues, problem.ValidationIssue{
				Field:   "headers",
				Message: "must not contain empty header keys",
			})
			break
		}
	}

	if len(issues) == 0 {
		return nil
	}

	return problem.Validation(problem.InvalidArgument, "envelope is invalid", issues...)
}

func newID() string {
	var raw [16]byte
	if _, err := rand.Read(raw[:]); err != nil {
		return time.Now().UTC().Format("20060102150405.000000000")
	}
	return hex.EncodeToString(raw[:])
}

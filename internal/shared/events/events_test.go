package events

import (
	"context"
	"errors"
	"testing"
	"time"
)

type testEvent struct {
	metadata Metadata
}

func (e testEvent) EventName() Name {
	return "test.event"
}

func (e testEvent) EventMetadata() Metadata {
	return e.metadata
}

func TestValidate(t *testing.T) {
	t.Parallel()

	if prob := Validate(testEvent{metadata: NewMetadata()}); prob != nil {
		t.Fatalf("expected valid event, got %v", prob)
	}

	if prob := Validate(testEvent{}); prob == nil {
		t.Fatal("expected invalid event metadata")
	}
}

func TestDispatcherDispatchesRegisteredHandlers(t *testing.T) {
	t.Parallel()

	dispatcher := NewDispatcher()
	event := testEvent{metadata: NewMetadata().WithOccurredAt(time.Unix(10, 0))}

	calls := 0
	dispatcher.Register(event.EventName(), HandlerFunc(func(_ context.Context, got Event) error {
		calls++

		typed, ok := got.(testEvent)
		if !ok {
			t.Fatalf("expected testEvent, got %T", got)
		}

		if typed.EventMetadata().ID != event.EventMetadata().ID {
			t.Fatalf("expected event id %q, got %q", event.EventMetadata().ID, typed.EventMetadata().ID)
		}

		return nil
	}))

	if err := dispatcher.Dispatch(context.Background(), event); err != nil {
		t.Fatalf("dispatch event: %v", err)
	}

	if calls != 1 {
		t.Fatalf("expected 1 handler call, got %d", calls)
	}
}

func TestDispatcherAggregatesHandlerErrors(t *testing.T) {
	t.Parallel()

	dispatcher := NewDispatcher()
	event := testEvent{metadata: NewMetadata()}

	expected := errors.New("boom")
	dispatcher.Register(event.EventName(), HandlerFunc(func(context.Context, Event) error {
		return expected
	}))

	err := dispatcher.Dispatch(context.Background(), event)
	if !errors.Is(err, expected) {
		t.Fatalf("expected joined error to contain %v, got %v", expected, err)
	}
}

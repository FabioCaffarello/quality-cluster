package nats

import (
	"context"
	"testing"

	"internal/shared/envelope"
	"internal/shared/problem"
)

func mustEncodeRequest[T any](t *testing.T, spec ControlSpec, payload T) []byte {
	t.Helper()

	data, prob := encodeControlRequest(context.Background(), spec, "test", payload)
	if prob != nil {
		t.Fatalf("encode request: %v", prob)
	}
	return data
}

func mustDecodeRequest[T any](t *testing.T, spec ControlSpec, payload []byte) envelope.Envelope[T] {
	t.Helper()

	env, prob := decodeControlRequest[T](spec, payload)
	if prob != nil {
		t.Fatalf("decode request: %v", prob)
	}

	return env
}

func problemUnavailable() *problem.Problem {
	return problem.New(problem.Unavailable, "nats unavailable")
}

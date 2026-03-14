package nats

import (
	"context"

	"internal/shared/envelope"
	"internal/shared/problem"
	"internal/shared/requestctx"

	"github.com/fxamacker/cbor/v2"
)

func encodeControlRequest[T any](ctx context.Context, spec ControlSpec, source string, payload T) ([]byte, *problem.Problem) {
	env := envelope.New(envelope.KindCommand, spec.RequestType, payload).
		WithSource(source).
		WithSubject(spec.Subject).
		WithContentType(contentTypeCBOR).
		WithCorrelationID(requestctx.CorrelationID(ctx))

	if prob := env.Validate(); prob != nil {
		return nil, prob
	}

	data, err := cbor.Marshal(env)
	if err != nil {
		return nil, problem.Wrap(err, problem.Internal, "failed to encode control request")
	}

	return data, nil
}

func decodeControlRequest[T any](spec ControlSpec, data []byte) (envelope.Envelope[T], *problem.Problem) {
	var env envelope.Envelope[T]
	if err := cbor.Unmarshal(data, &env); err != nil {
		return env, problem.Wrap(err, problem.InvalidArgument, "invalid control request")
	}

	if env.Kind != envelope.KindCommand {
		return env, problem.New(problem.InvalidArgument, "control request kind is invalid")
	}
	if env.Type != spec.RequestType {
		return env, problem.New(problem.InvalidArgument, "control request type is invalid")
	}
	if prob := env.Validate(); prob != nil {
		return env, prob
	}

	return env, nil
}

func encodeControlReply[Req any, Res any](spec ControlSpec, source string, request envelope.Envelope[Req], reply Res, prob *problem.Problem) ([]byte, error) {
	env := envelope.New(envelope.KindReply, spec.ReplyType, reply).
		WithSource(source).
		WithContentType(contentTypeCBOR).
		WithCorrelationID(request.CorrelationID).
		WithCausationID(request.ID).
		WithProblem(prob)

	return cbor.Marshal(env)
}

func decodeControlReply[T any](spec ControlSpec, data []byte) (T, *problem.Problem) {
	var zero T
	var env envelope.Envelope[T]
	if err := cbor.Unmarshal(data, &env); err != nil {
		return zero, problem.Wrap(err, problem.Internal, "failed to decode control reply")
	}

	if env.Kind != envelope.KindReply {
		return zero, problem.New(problem.Internal, "control reply kind is invalid")
	}
	if env.Type != spec.ReplyType {
		return zero, problem.New(problem.Internal, "control reply type is invalid")
	}
	if env.Problem != nil {
		return zero, env.Problem
	}

	return env.Payload, nil
}

func encodeEvent[T any](spec EventSpec, source string, payload T, correlationID string) ([]byte, *problem.Problem) {
	env := envelope.New(envelope.KindEvent, spec.Type, payload).
		WithSource(source).
		WithSubject(spec.Subject).
		WithContentType(contentTypeCBOR).
		WithCorrelationID(correlationID)

	if prob := env.Validate(); prob != nil {
		return nil, prob
	}

	data, err := cbor.Marshal(env)
	if err != nil {
		return nil, problem.Wrap(err, problem.Internal, "failed to encode runtime event")
	}

	return data, nil
}

func decodeEvent[T any](spec EventSpec, data []byte) (envelope.Envelope[T], *problem.Problem) {
	var env envelope.Envelope[T]
	if err := cbor.Unmarshal(data, &env); err != nil {
		return env, problem.Wrap(err, problem.InvalidArgument, "invalid runtime event")
	}

	if env.Kind != envelope.KindEvent {
		return env, problem.New(problem.InvalidArgument, "runtime event kind is invalid")
	}
	if env.Type != spec.Type {
		return env, problem.New(problem.InvalidArgument, "runtime event type is invalid")
	}
	if prob := env.Validate(); prob != nil {
		return env, prob
	}

	return env, nil
}

type ControlRoute struct {
	Spec    ControlSpec
	Handler func(context.Context, []byte) ([]byte, error)
}

func NewTypedControlRoute[Req any, Res any](spec ControlSpec, source string, handler func(context.Context, Req) (Res, *problem.Problem)) ControlRoute {
	return ControlRoute{
		Spec: spec,
		Handler: func(ctx context.Context, payload []byte) ([]byte, error) {
			request, prob := decodeControlRequest[Req](spec, payload)
			if prob != nil {
				var zero Res
				return encodeControlReply(spec, source, request, zero, prob)
			}

			replyCtx := requestctx.WithCorrelationID(ctx, request.CorrelationID)
			reply, prob := handler(replyCtx, request.Payload)
			return encodeControlReply(spec, source, request, reply, prob)
		},
	}
}

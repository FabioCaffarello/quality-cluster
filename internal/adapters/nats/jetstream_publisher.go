package nats

import (
	"context"
	"fmt"

	"internal/application/configctl/contracts"
	"internal/shared/problem"

	"github.com/nats-io/nats.go"
	"github.com/nats-io/nats.go/jetstream"
)

type RuntimeEventPublisher struct {
	url      string
	source   string
	registry ConfigctlRegistry
	nc       *nats.Conn
	js       jetstream.JetStream
}

func NewRuntimeEventPublisher(url, source string, registry ConfigctlRegistry) *RuntimeEventPublisher {
	return &RuntimeEventPublisher{
		url:      url,
		source:   source,
		registry: registry,
	}
}

func (p *RuntimeEventPublisher) Start() error {
	nc, err := connect(p.url)
	if err != nil {
		return err
	}

	js, err := jetstream.New(nc)
	if err != nil {
		nc.Close()
		return fmt.Errorf("create jetstream context: %w", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), defaultSetupTimeout)
	defer cancel()

	if _, err := js.CreateOrUpdateStream(ctx, p.registry.RuntimeUpdated.Stream.Config()); err != nil {
		nc.Close()
		return fmt.Errorf("ensure runtime stream: %w", err)
	}

	p.nc = nc
	p.js = js
	return nil
}

func (p *RuntimeEventPublisher) Publish(ctx context.Context, event contracts.RuntimeEvent) *problem.Problem {
	if p == nil || p.js == nil {
		return problem.New(problem.Unavailable, "runtime event publisher is unavailable")
	}

	switch typed := event.(type) {
	case contracts.RuntimeUpdatedEvent:
		return p.publishRuntimeUpdated(ctx, typed)
	default:
		return problem.New(problem.InvalidArgument, "runtime event type is unsupported")
	}
}

func (p *RuntimeEventPublisher) publishRuntimeUpdated(ctx context.Context, event contracts.RuntimeUpdatedEvent) *problem.Problem {
	data, prob := encodeRuntimeEvent(p.registry.RuntimeUpdated, p.source, event, event.Metadata.CorrelationID)
	if prob != nil {
		return prob
	}

	if _, err := p.js.Publish(ctx, p.registry.RuntimeUpdated.Subject, data, jetstream.WithMsgID(event.Metadata.ID)); err != nil {
		return problem.Wrap(err, problem.Unavailable, "publish runtime update")
	}

	return nil
}

func (p *RuntimeEventPublisher) Close() error {
	if p != nil && p.nc != nil {
		p.nc.Close()
	}
	return nil
}

package nats

import (
	"context"
	"fmt"

	dataplaneapp "internal/application/dataplane"
	"internal/shared/problem"

	"github.com/nats-io/nats.go"
	"github.com/nats-io/nats.go/jetstream"
)

type DataPlanePublisher struct {
	url      string
	source   string
	registry DataPlaneRegistry
	nc       *nats.Conn
	js       jetstream.JetStream
}

func NewDataPlanePublisher(url, source string, registry DataPlaneRegistry) *DataPlanePublisher {
	return &DataPlanePublisher{
		url:      url,
		source:   source,
		registry: registry,
	}
}

func (p *DataPlanePublisher) Start() error {
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

	if _, err := js.CreateOrUpdateStream(ctx, p.registry.Ingested.Stream.Config()); err != nil {
		nc.Close()
		return fmt.Errorf("ensure data plane stream: %w", err)
	}

	p.nc = nc
	p.js = js
	return nil
}

func (p *DataPlanePublisher) Publish(ctx context.Context, subject string, correlationID string, payload dataplaneapp.Message) *problem.Problem {
	if p == nil || p.js == nil {
		return problem.New(problem.Unavailable, "data plane publisher is unavailable")
	}

	spec := p.registry.EventSpec(subject)
	data, prob := encodeEvent(spec, p.source, payload, correlationID)
	if prob != nil {
		return prob
	}

	if _, err := p.js.Publish(ctx, subject, data, jetstream.WithMsgID(payload.MessageID())); err != nil {
		return problem.Wrap(err, problem.Unavailable, "publish data plane message")
	}
	return nil
}

func (p *DataPlanePublisher) Close() error {
	if p != nil && p.nc != nil {
		p.nc.Close()
	}
	return nil
}

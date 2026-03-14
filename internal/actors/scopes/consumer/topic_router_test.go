package consumer

import (
	"testing"
	"time"

	actorcommon "internal/actors/common"
	adapterkafka "internal/adapters/kafka"
	configctlcontracts "internal/application/configctl/contracts"
	dataplaneapp "internal/application/dataplane"
	sharedruntime "internal/application/runtimecontracts"

	"github.com/anthdm/hollywood/actor"
)

type capturePublishedMessagesQuery struct{}

type capturePublishedMessagesResult struct {
	Messages []dataplaneapp.RoutedMessage
}

type publisherProbeActor struct {
	messages []dataplaneapp.RoutedMessage
}

func (a *publisherProbeActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case publishRoutedMessageMessage:
		a.messages = append(a.messages, msg.Message)
		c.Respond(publishRoutedMessageResult{})
	case capturePublishedMessagesQuery:
		c.Respond(capturePublishedMessagesResult{Messages: append([]dataplaneapp.RoutedMessage(nil), a.messages...)})
	}
}

func TestTopicRouterActorRoutesAllBindingsForTopic(t *testing.T) {
	t.Parallel()

	engine, err := actorcommon.NewDefaultEngine()
	if err != nil {
		t.Fatalf("new engine: %v", err)
	}

	publisherPID := engine.Spawn(func() actor.Receiver { return &publisherProbeActor{} }, "publisher-probe")
	routerPID := engine.Spawn(NewTopicRouterActor(TopicRouterConfig{
		Topic: dataplaneapp.TopicTopology{
			Topic: "sales.order.created",
			Bindings: []dataplaneapp.RoutedBinding{
				{
					Binding: configctlcontracts.ActiveIngestionBindingRecord{
						Binding: configctlcontracts.BindingRecord{Name: "orders", Topic: "sales.order.created"},
						Runtime: sharedruntime.RuntimeRecord{
							Scope:  sharedruntime.ScopeRecord{Kind: "global", Key: "default"},
							Config: sharedruntime.ConfigRecord{VersionID: "cfg-1", DefinitionChecksum: "sum-1"},
						},
					},
					Route: dataplaneapp.BindingRoute{
						KafkaTopic:       "sales.order.created",
						JetStreamSubject: "dataplane.ingestion.received.global.default.orders",
					},
				},
				{
					Binding: configctlcontracts.ActiveIngestionBindingRecord{
						Binding: configctlcontracts.BindingRecord{Name: "orders-audit", Topic: "sales.order.created"},
						Runtime: sharedruntime.RuntimeRecord{
							Scope:  sharedruntime.ScopeRecord{Kind: "tenant", Key: "br"},
							Config: sharedruntime.ConfigRecord{VersionID: "cfg-2", DefinitionChecksum: "sum-2"},
						},
					},
					Route: dataplaneapp.BindingRoute{
						KafkaTopic:       "sales.order.created",
						JetStreamSubject: "dataplane.ingestion.received.tenant.br.orders-audit",
					},
				},
			},
		},
		PublisherPID:   publisherPID,
		RequestTimeout: time.Second,
	}), "topic-router-test")

	result, err := engine.Request(routerPID, routeKafkaMessageMessage{
		Message: adapterkafka.Message{
			Topic:     "sales.order.created",
			Key:       []byte("order-1"),
			Value:     []byte(`{"order_id":"1"}`),
			Headers:   map[string]string{"x-correlation-id": "corr-1"},
			Partition: 1,
			Offset:    10,
			Timestamp: time.Unix(10, 0).UTC(),
		},
		IngestedAt: time.Unix(20, 0).UTC(),
	}, time.Second).Result()
	if err != nil {
		t.Fatalf("route kafka message: %v", err)
	}

	reply := result.(routeKafkaMessageResult)
	if reply.Prob != nil {
		t.Fatalf("expected no problem, got %v", reply.Prob)
	}

	publishedRaw, err := engine.Request(publisherPID, capturePublishedMessagesQuery{}, time.Second).Result()
	if err != nil {
		t.Fatalf("query published messages: %v", err)
	}

	published := publishedRaw.(capturePublishedMessagesResult)
	if len(published.Messages) != 2 {
		t.Fatalf("expected two published messages, got %+v", published.Messages)
	}
	if published.Messages[0].Route.JetStreamSubject != "dataplane.ingestion.received.global.default.orders" {
		t.Fatalf("unexpected first subject: %+v", published.Messages[0].Route)
	}
	if published.Messages[1].Route.JetStreamSubject != "dataplane.ingestion.received.tenant.br.orders-audit" {
		t.Fatalf("unexpected second subject: %+v", published.Messages[1].Route)
	}
}

package consumer

import (
	"fmt"
	"log/slog"
	"strings"

	actorcommon "internal/actors/common"
	adapterkafka "internal/adapters/kafka"
	adapternats "internal/adapters/nats"
	runtimebootstrap "internal/application/runtimebootstrap"
	"internal/shared/settings"

	"github.com/anthdm/hollywood/actor"
)

type ConsumerRuntimeConfig struct {
	AppConfig         settings.AppConfig
	Generation        int
	Bootstrap         runtimebootstrap.ActiveIngestionBootstrap
	DataPlaneRegistry adapternats.DataPlaneRegistry
	Source            string
}

type ConsumerRuntimeActor struct {
	cfg          ConsumerRuntimeConfig
	logger       *slog.Logger
	publisherPID *actor.PID
	routerPIDs   map[string]*actor.PID
}

func NewConsumerRuntimeActor(cfg ConsumerRuntimeConfig) actor.Producer {
	return func() actor.Receiver {
		return &ConsumerRuntimeActor{
			cfg:        cfg,
			logger:     slog.Default(),
			routerPIDs: make(map[string]*actor.PID),
		}
	}
}

func (a *ConsumerRuntimeActor) Receive(c *actor.Context) {
	switch msg := c.Message().(type) {
	case actor.Started:
		if a.cfg.Bootstrap.Topology.BindingCount() == 0 {
			a.fail(c, fmt.Errorf("bootstrap topology is unavailable"))
			return
		}

		a.publisherPID = c.SpawnChild(NewDataPlanePublisherActor(DataPlanePublisherConfig{
			URL:      a.cfg.AppConfig.NATS.URL,
			Source:   a.cfg.Source,
			Registry: a.cfg.DataPlaneRegistry,
		}), "publisher")

		for _, topic := range a.cfg.Bootstrap.Topology.Topics() {
			a.routerPIDs[topic.Topic] = c.SpawnChild(NewTopicRouterActor(TopicRouterConfig{
				Topic:          topic,
				PublisherPID:   a.publisherPID,
				RequestTimeout: a.cfg.AppConfig.NATS.RequestTimeoutDuration(),
			}), topicRouterActorName(topic.Topic))
		}
	case dataPlanePublisherReadyMessage:
		a.startTopicConsumers(c)
	case dataPlanePublisherFailedMessage:
		a.fail(c, msg.Err)
	case kafkaTopicConsumerFailedMessage:
		a.fail(c, fmt.Errorf("topic %s: %w", msg.Topic, msg.Err))
	case actor.Stopped:
	default:
		if actorcommon.ShouldIgnoreLifecycleMessage(msg) {
			return
		}
		a.logger.Warn("consumer runtime: unknown message", "type", fmt.Sprintf("%T", msg))
	}
}

func (a *ConsumerRuntimeActor) startTopicConsumers(c *actor.Context) {
	topology := a.cfg.Bootstrap.Topology
	if len(topology.Topics()) == 0 {
		return
	}

	if err := adapterkafka.EnsureTopics(c.Context(), a.cfg.AppConfig.Kafka.Brokers, topology.TopicNames(), a.cfg.AppConfig.Kafka.DialTimeoutDuration()); err != nil {
		a.fail(c, err)
		return
	}

	if a.publisherPID == nil {
		a.fail(c, fmt.Errorf("publisher actor is unavailable"))
		return
	}

	for _, topic := range topology.Topics() {
		routerPID := a.routerPIDs[topic.Topic]
		if routerPID == nil {
			a.fail(c, fmt.Errorf("router for topic %s is unavailable", topic.Topic))
			return
		}

		c.SpawnChild(NewKafkaTopicConsumerActor(KafkaTopicConsumerConfig{
			Kafka:          a.cfg.AppConfig.Kafka,
			Topic:          topic.Topic,
			RouterPID:      routerPID,
			RequestTimeout: a.cfg.AppConfig.NATS.RequestTimeoutDuration(),
		}), topicConsumerActorName(topic.Topic))
	}

	c.Send(c.Parent(), consumerRuntimeReadyMessage{
		Generation:         a.cfg.Generation,
		Topology:           topology,
		BootstrapSignature: a.cfg.Bootstrap.Signature(),
		RuntimeRefs:        a.cfg.Bootstrap.RuntimeRefs(),
	})
}

func (a *ConsumerRuntimeActor) fail(c *actor.Context, err error) {
	c.Send(c.Parent(), consumerRuntimeFailedMessage{
		Generation: a.cfg.Generation,
		Err:        err,
	})
	c.Engine().Poison(c.PID())
}

func topicRouterActorName(topic string) string {
	return "router-" + actorNameToken(topic)
}

func topicConsumerActorName(topic string) string {
	return "consumer-" + actorNameToken(topic)
}

func actorNameToken(raw string) string {
	raw = strings.ToLower(strings.TrimSpace(raw))
	if raw == "" {
		return "default"
	}

	var builder strings.Builder
	lastDash := false
	for _, char := range raw {
		if (char >= 'a' && char <= 'z') || (char >= '0' && char <= '9') {
			builder.WriteRune(char)
			lastDash = false
			continue
		}
		if lastDash {
			continue
		}
		builder.WriteByte('-')
		lastDash = true
	}

	token := strings.Trim(builder.String(), "-")
	if token == "" {
		return "default"
	}
	return token
}

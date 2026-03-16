package consumer

import (
	"time"

	adapterkafka "internal/adapters/kafka"
	dataplaneapp "internal/application/dataplane"
	runtimebootstrap "internal/application/runtimebootstrap"
	configdomain "internal/domain/configctl"
	"internal/shared/problem"
)

type activeIngestionBootstrapLoadedMessage struct {
	Bootstrap runtimebootstrap.ActiveIngestionBootstrap
}

type activeIngestionBootstrapFailedMessage struct {
	Prob *problem.Problem
}

type refreshActiveIngestionBootstrapMessage struct {
	Event configdomain.IngestionRuntimeChangedEvent
}

type consumerRuntimeReadyMessage struct {
	Generation int
	Topology   dataplaneapp.RuntimeTopology
}

type consumerRuntimeFailedMessage struct {
	Generation int
	Err        error
}

type dataPlanePublisherReadyMessage struct{}

type dataPlanePublisherFailedMessage struct {
	Err error
}

type kafkaTopicConsumerFailedMessage struct {
	Topic string
	Err   error
}

type routeKafkaMessageMessage struct {
	Message    adapterkafka.Message
	IngestedAt time.Time
}

type routeKafkaMessageResult struct {
	Prob *problem.Problem
}

type publishRoutedMessageMessage struct {
	Message dataplaneapp.RoutedMessage
}

type publishRoutedMessageResult struct {
	Prob *problem.Problem
}

type queryConsumerSupervisorStateMessage struct{}

type queryConsumerSupervisorStateResult struct {
	State ConsumerSupervisorState
}

type ConsumerSupervisorState struct {
	Generation int
	Ready      bool
	Topics     []string
	Bindings   int
}

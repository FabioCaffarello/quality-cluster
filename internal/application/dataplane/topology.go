package dataplane

import (
	"strings"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/shared/problem"
)

type RoutedBinding struct {
	Binding configctlcontracts.ActiveIngestionBindingRecord
	Route   BindingRoute
}

type TopicTopology struct {
	Topic    string
	Bindings []RoutedBinding
}

type RuntimeTopology struct {
	topics  []TopicTopology
	byTopic map[string]TopicTopology
}

func NewRuntimeTopology(index BindingIndex, registry Registry) (RuntimeTopology, *problem.Problem) {
	if len(index.All()) == 0 {
		return RuntimeTopology{}, problem.New(problem.NotFound, "no active ingestion bindings were found")
	}

	topology := RuntimeTopology{
		topics:  make([]TopicTopology, 0, len(index.Topics())),
		byTopic: make(map[string]TopicTopology, len(index.Topics())),
	}

	for _, topic := range index.Topics() {
		routedBindings := make([]RoutedBinding, 0, len(index.BindingsForTopic(topic)))
		for _, binding := range index.BindingsForTopic(topic) {
			route, prob := registry.RouteForBinding(binding)
			if prob != nil {
				return RuntimeTopology{}, prob
			}
			routedBindings = append(routedBindings, RoutedBinding{
				Binding: binding,
				Route:   route,
			})
		}

		topic = strings.TrimSpace(topic)
		entry := TopicTopology{
			Topic:    topic,
			Bindings: routedBindings,
		}
		topology.topics = append(topology.topics, entry)
		topology.byTopic[topic] = entry
	}

	return topology, nil
}

func (t RuntimeTopology) Topics() []TopicTopology {
	return append([]TopicTopology(nil), t.topics...)
}

func (t RuntimeTopology) TopicNames() []string {
	names := make([]string, 0, len(t.topics))
	for _, topic := range t.topics {
		names = append(names, topic.Topic)
	}
	return names
}

func (t RuntimeTopology) BindingsForTopic(topic string) []RoutedBinding {
	entry, ok := t.byTopic[strings.TrimSpace(topic)]
	if !ok {
		return nil
	}
	return append([]RoutedBinding(nil), entry.Bindings...)
}

func (t RuntimeTopology) BindingCount() int {
	total := 0
	for _, topic := range t.topics {
		total += len(topic.Bindings)
	}
	return total
}

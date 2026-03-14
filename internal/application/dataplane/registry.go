package dataplane

import (
	"strings"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/shared/problem"
)

const unknownRouteToken = "unknown"

type Registry struct {
	Kafka     KafkaRegistry
	JetStream JetStreamRegistry
}

type KafkaRegistry struct{}

type JetStreamRegistry struct {
	Ingested IngestedRoute
}

type IngestedRoute struct {
	Stream           string
	SubjectPrefix    string
	SubjectPattern   string
	EventType        string
	ValidatorDurable string
}

type BindingRoute struct {
	KafkaTopic       string
	JetStreamSubject string
}

func DefaultRegistry() Registry {
	const subjectPrefix = "dataplane.ingestion.received"

	return Registry{
		Kafka: KafkaRegistry{},
		JetStream: JetStreamRegistry{
			Ingested: IngestedRoute{
				Stream:           "DATA_PLANE_INGESTION",
				SubjectPrefix:    subjectPrefix,
				SubjectPattern:   subjectPrefix + ".>",
				EventType:        "dataplane.event.ingestion.received",
				ValidatorDurable: "validator-dataplane-v1",
			},
		},
	}
}

func (r Registry) RouteForBinding(binding configctlcontracts.ActiveIngestionBindingRecord) (BindingRoute, *problem.Problem) {
	topic := strings.TrimSpace(binding.Binding.Topic)
	if topic == "" {
		return BindingRoute{}, problem.New(problem.InvalidArgument, "binding topic is required")
	}

	subject, prob := r.SubjectForBinding(binding)
	if prob != nil {
		return BindingRoute{}, prob
	}

	return BindingRoute{
		KafkaTopic:       topic,
		JetStreamSubject: subject,
	}, nil
}

func (r Registry) SubjectForBinding(binding configctlcontracts.ActiveIngestionBindingRecord) (string, *problem.Problem) {
	prefix := strings.TrimSpace(r.JetStream.Ingested.SubjectPrefix)
	if prefix == "" {
		return "", problem.New(problem.InvalidArgument, "jetstream subject prefix is required")
	}

	name := strings.TrimSpace(binding.Binding.Name)
	scopeKind := strings.TrimSpace(binding.Runtime.Scope.Kind)
	scopeKey := strings.TrimSpace(binding.Runtime.Scope.Key)
	if name == "" || scopeKind == "" || scopeKey == "" {
		return "", problem.New(problem.InvalidArgument, "binding route identifiers are incomplete")
	}

	return strings.Join([]string{
		prefix,
		sanitizeToken(scopeKind),
		sanitizeToken(scopeKey),
		sanitizeToken(name),
	}, "."), nil
}

func sanitizeToken(raw string) string {
	raw = strings.ToLower(strings.TrimSpace(raw))
	if raw == "" {
		return unknownRouteToken
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
		return unknownRouteToken
	}
	return token
}

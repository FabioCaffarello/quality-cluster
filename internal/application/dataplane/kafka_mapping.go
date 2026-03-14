package dataplane

import (
	"strings"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/shared/problem"
)

type KafkaRecord struct {
	Topic       string
	Key         string
	Payload     []byte
	Headers     map[string]string
	Partition   int
	Offset      int64
	PublishedAt time.Time
}

type RoutedMessage struct {
	Route         BindingRoute
	CorrelationID string
	Message       Message
}

func NewKafkaRecord(topic string, key []byte, payload []byte, headers map[string]string, partition int, offset int64, publishedAt time.Time) (KafkaRecord, *problem.Problem) {
	record := KafkaRecord{
		Topic:       strings.TrimSpace(topic),
		Key:         strings.TrimSpace(string(key)),
		Payload:     append([]byte(nil), payload...),
		Headers:     normalizedHeaders(headers),
		Partition:   partition,
		Offset:      offset,
		PublishedAt: publishedAt.UTC(),
	}

	var issues []problem.ValidationIssue
	if record.Topic == "" {
		issues = append(issues, problem.ValidationIssue{Field: "topic", Message: "must not be empty"})
	}
	if len(record.Payload) == 0 {
		issues = append(issues, problem.ValidationIssue{Field: "payload", Message: "must not be empty"})
	}
	if record.Partition < 0 {
		issues = append(issues, problem.ValidationIssue{Field: "partition", Message: "must not be negative"})
	}
	if record.Offset < 0 {
		issues = append(issues, problem.ValidationIssue{Field: "offset", Message: "must not be negative"})
	}
	if len(issues) > 0 {
		return KafkaRecord{}, problem.Validation(problem.InvalidArgument, "kafka record is invalid", issues...)
	}

	if record.PublishedAt.IsZero() {
		record.PublishedAt = time.Now().UTC()
	}

	return record, nil
}

func MapKafkaRecord(binding configctlcontracts.ActiveIngestionBindingRecord, registry Registry, record KafkaRecord, ingestedAt time.Time) (RoutedMessage, *problem.Problem) {
	route, prob := registry.RouteForBinding(binding)
	if prob != nil {
		return RoutedMessage{}, prob
	}

	return MapKafkaRecordToBinding(RoutedBinding{
		Binding: binding,
		Route:   route,
	}, record, ingestedAt)
}

func MapKafkaRecordToBinding(binding RoutedBinding, record KafkaRecord, ingestedAt time.Time) (RoutedMessage, *problem.Problem) {
	if strings.TrimSpace(binding.Route.JetStreamSubject) == "" {
		return RoutedMessage{}, problem.New(problem.InvalidArgument, "binding route jetstream subject is required")
	}

	messageID := MessageIDForKafkaRecord(binding.Binding, record.Topic, record.Partition, record.Offset)
	correlationID := strings.TrimSpace(record.Headers["x-correlation-id"])
	if correlationID == "" {
		correlationID = messageID
	}

	message, prob := NewMessage(binding.Binding, record.Payload, OriginRecord{
		Source:      SourceKafka,
		Topic:       record.Topic,
		Key:         record.Key,
		PublishedAt: record.PublishedAt,
	}, MetadataRecord{
		MessageID:     messageID,
		CorrelationID: correlationID,
		IngestedAt:    ingestedAt,
		ContentType:   record.Headers["content-type"],
	})
	if prob != nil {
		return RoutedMessage{}, prob
	}

	return RoutedMessage{
		Route:         binding.Route,
		CorrelationID: correlationID,
		Message:       message,
	}, nil
}

func normalizedHeaders(headers map[string]string) map[string]string {
	if len(headers) == 0 {
		return nil
	}

	result := make(map[string]string, len(headers))
	for key, value := range headers {
		key = strings.TrimSpace(key)
		if key == "" {
			continue
		}
		result[key] = strings.TrimSpace(value)
	}
	if len(result) == 0 {
		return nil
	}
	return result
}

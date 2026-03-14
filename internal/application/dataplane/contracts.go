package dataplane

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"

	configctlcontracts "internal/application/configctl/contracts"
	"internal/shared/problem"
)

const (
	SourceKafka        = "kafka"
	ContentTypeJSON    = "application/json"
	defaultProblemText = "data plane message is invalid"
)

type ScopeRecord struct {
	Kind string `json:"kind"`
	Key  string `json:"key"`
}

type ConfigRecord struct {
	SetID              string `json:"set_id"`
	Key                string `json:"key"`
	VersionID          string `json:"version_id"`
	Version            int    `json:"version"`
	DefinitionChecksum string `json:"definition_checksum"`
}

type BindingRecord struct {
	Name   string       `json:"name"`
	Topic  string       `json:"topic"`
	Scope  ScopeRecord  `json:"scope"`
	Config ConfigRecord `json:"config"`
}

type OriginRecord struct {
	Source      string    `json:"source"`
	Topic       string    `json:"topic"`
	Key         string    `json:"key,omitempty"`
	PublishedAt time.Time `json:"published_at,omitempty"`
}

type MetadataRecord struct {
	MessageID     string    `json:"message_id"`
	CorrelationID string    `json:"correlation_id,omitempty"`
	IngestedAt    time.Time `json:"ingested_at"`
	ContentType   string    `json:"content_type,omitempty"`
}

type Message struct {
	Binding  BindingRecord   `json:"binding"`
	Origin   OriginRecord    `json:"origin"`
	Payload  json.RawMessage `json:"payload"`
	Metadata MetadataRecord  `json:"metadata"`
}

func NewMessage(binding configctlcontracts.ActiveIngestionBindingRecord, payload []byte, origin OriginRecord, metadata MetadataRecord) (Message, *problem.Problem) {
	message := Message{
		Binding: bindingRecordFromActiveBinding(binding),
		Origin: OriginRecord{
			Source:      strings.ToLower(strings.TrimSpace(origin.Source)),
			Topic:       strings.TrimSpace(origin.Topic),
			Key:         strings.TrimSpace(origin.Key),
			PublishedAt: origin.PublishedAt.UTC(),
		},
		Payload: append(json.RawMessage(nil), payload...),
		Metadata: MetadataRecord{
			MessageID:     strings.TrimSpace(metadata.MessageID),
			CorrelationID: strings.TrimSpace(metadata.CorrelationID),
			IngestedAt:    metadata.IngestedAt.UTC(),
			ContentType:   strings.TrimSpace(metadata.ContentType),
		},
	}

	if message.Metadata.IngestedAt.IsZero() {
		message.Metadata.IngestedAt = time.Now().UTC()
	}
	if message.Metadata.ContentType == "" {
		message.Metadata.ContentType = ContentTypeJSON
	}
	if message.Origin.Source == "" {
		message.Origin.Source = SourceKafka
	}

	if prob := message.Validate(); prob != nil {
		return Message{}, prob
	}

	return message, nil
}

func (m Message) Validate() *problem.Problem {
	var issues []problem.ValidationIssue

	if strings.TrimSpace(m.Binding.Name) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "binding.name", Message: "must not be empty"})
	}
	if strings.TrimSpace(m.Binding.Topic) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "binding.topic", Message: "must not be empty"})
	}
	if strings.TrimSpace(m.Binding.Scope.Kind) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "binding.scope.kind", Message: "must not be empty"})
	}
	if strings.TrimSpace(m.Binding.Scope.Key) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "binding.scope.key", Message: "must not be empty"})
	}
	if strings.TrimSpace(m.Binding.Config.VersionID) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "binding.config.version_id", Message: "must not be empty"})
	}
	if strings.TrimSpace(m.Origin.Source) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "origin.source", Message: "must not be empty"})
	}
	if strings.TrimSpace(m.Origin.Topic) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "origin.topic", Message: "must not be empty"})
	}
	if strings.TrimSpace(m.Metadata.MessageID) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "metadata.message_id", Message: "must not be empty"})
	}
	if m.Metadata.IngestedAt.IsZero() {
		issues = append(issues, problem.ValidationIssue{Field: "metadata.ingested_at", Message: "must not be zero"})
	}
	if strings.TrimSpace(m.Metadata.ContentType) == "" {
		issues = append(issues, problem.ValidationIssue{Field: "metadata.content_type", Message: "must not be empty"})
	}
	if len(m.Payload) == 0 {
		issues = append(issues, problem.ValidationIssue{Field: "payload", Message: "must not be empty"})
	} else if !json.Valid(m.Payload) {
		issues = append(issues, problem.ValidationIssue{Field: "payload", Message: "must be valid JSON"})
	}
	if len(issues) == 0 {
		return nil
	}

	return problem.Validation(problem.InvalidArgument, defaultProblemText, issues...)
}

func (m Message) MessageID() string {
	return strings.TrimSpace(m.Metadata.MessageID)
}

func bindingRecordFromActiveBinding(binding configctlcontracts.ActiveIngestionBindingRecord) BindingRecord {
	return BindingRecord{
		Name:  strings.TrimSpace(binding.Binding.Name),
		Topic: strings.TrimSpace(binding.Binding.Topic),
		Scope: ScopeRecord{
			Kind: strings.TrimSpace(binding.Runtime.Scope.Kind),
			Key:  strings.TrimSpace(binding.Runtime.Scope.Key),
		},
		Config: ConfigRecord{
			SetID:              strings.TrimSpace(binding.Runtime.Config.SetID),
			Key:                strings.TrimSpace(binding.Runtime.Config.Key),
			VersionID:          strings.TrimSpace(binding.Runtime.Config.VersionID),
			Version:            binding.Runtime.Config.Version,
			DefinitionChecksum: strings.TrimSpace(binding.Runtime.Config.DefinitionChecksum),
		},
	}
}

func MessageIDForKafkaRecord(binding configctlcontracts.ActiveIngestionBindingRecord, topic string, partition int, offset int64) string {
	return fmt.Sprintf(
		"%s:%s:%d:%d:%s:%s:%s",
		SourceKafka,
		strings.TrimSpace(topic),
		partition,
		offset,
		strings.TrimSpace(binding.Runtime.Scope.Kind)+":"+strings.TrimSpace(binding.Runtime.Scope.Key),
		strings.TrimSpace(binding.Runtime.Config.VersionID),
		strings.TrimSpace(binding.Binding.Name),
	)
}

package kafka

import (
	"strings"
	"time"

	kafkago "github.com/segmentio/kafka-go"
)

type Message struct {
	Topic     string
	Key       []byte
	Value     []byte
	Headers   map[string]string
	Partition int
	Offset    int64
	Timestamp time.Time
}

func messageFromRecord(record kafkago.Message) Message {
	return Message{
		Topic:     strings.TrimSpace(record.Topic),
		Key:       append([]byte(nil), record.Key...),
		Value:     append([]byte(nil), record.Value...),
		Headers:   headerMap(record.Headers),
		Partition: record.Partition,
		Offset:    record.Offset,
		Timestamp: record.Time.UTC(),
	}
}

func kafkaHeaders(headers map[string]string) []kafkago.Header {
	if len(headers) == 0 {
		return nil
	}

	result := make([]kafkago.Header, 0, len(headers))
	for key, value := range headers {
		key = strings.TrimSpace(key)
		if key == "" {
			continue
		}
		result = append(result, kafkago.Header{
			Key:   key,
			Value: []byte(strings.TrimSpace(value)),
		})
	}
	return result
}

func headerMap(headers []kafkago.Header) map[string]string {
	if len(headers) == 0 {
		return nil
	}

	result := make(map[string]string, len(headers))
	for _, header := range headers {
		key := strings.TrimSpace(header.Key)
		if key == "" {
			continue
		}
		result[key] = strings.TrimSpace(string(header.Value))
	}
	return result
}

package configctl

import (
	"internal/application/configctl/contracts"
	"internal/domain/configuration"
)

func recordFromDomain(config configuration.Config) contracts.ConfigRecord {
	return contracts.ConfigRecord{
		ID:        config.ID,
		Name:      config.Name,
		Format:    string(config.Format),
		Content:   config.Content,
		Status:    string(config.Status),
		CreatedAt: config.CreatedAt,
		UpdatedAt: config.UpdatedAt,
	}
}

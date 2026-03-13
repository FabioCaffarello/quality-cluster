package contracts

type CreateDraftReply struct {
	Config ConfigRecord `json:"config"`
}

type GetConfigReply struct {
	Config ConfigRecord `json:"config"`
}

type GetActiveConfigReply struct {
	Config ConfigRecord `json:"config"`
}

type ListConfigsReply struct {
	Configs []ConfigRecord `json:"configs"`
}

type ValidationDiagnostic struct {
	Field   string `json:"field,omitempty"`
	Message string `json:"message"`
}

type ValidateDraftReply struct {
	Valid       bool                   `json:"valid"`
	Diagnostics []ValidationDiagnostic `json:"diagnostics,omitempty"`
}

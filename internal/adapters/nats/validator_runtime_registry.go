package nats

type ValidatorRuntimeRegistry struct {
	GetActive ControlSpec
}

func DefaultValidatorRuntimeRegistry() ValidatorRuntimeRegistry {
	return ValidatorRuntimeRegistry{
		GetActive: ControlSpec{
			Subject:     "validator.runtime.get_active",
			RequestType: "validator.runtime.query.get_active",
			ReplyType:   "validator.runtime.reply.get_active",
			QueueGroup:  "validator.runtime",
		},
	}
}

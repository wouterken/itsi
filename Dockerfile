FROM ruby:3.4.2-slim

RUN apt-get update && apt-get install -y libclang-dev build-essential curl \
  && apt-get clean && rm -rf /var/lib/apt/lists/*
RUN gem install itsi

CMD ["itsi" ]

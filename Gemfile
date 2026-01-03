# frozen_string_literal: true

source 'https://rubygems.org'

git_source(:github) { |repo_name| "https://github.com/#{repo_name}" }

gem 'aws-sdk-dynamodb', '~> 1.157'
gem 'aws-sdk-ses', '~> 1.93'
gem 'http', '~> 5.3'
gem 'nokogiri', '~> 1.19' # Peer requirement of aws-sdk

group :development do
  gem 'pry-byebug', '~> 3.11'
  gem 'rubocop', '~> 1.82', require: false
end

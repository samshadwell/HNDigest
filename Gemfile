# frozen_string_literal: true

source 'https://rubygems.org'

git_source(:github) { |repo_name| "https://github.com/#{repo_name}" }

gem 'aws-sdk-dynamodb', '~> 1.155'
gem 'aws-sdk-ses', '~> 1.92'
gem 'http', '~> 5.3'
gem 'nokogiri', '~> 1.18' # Peer requirement of aws-sdk

group :development do
  gem 'pry-byebug', '~> 3.11'
  gem 'rubocop', '~> 1.81', require: false
end
